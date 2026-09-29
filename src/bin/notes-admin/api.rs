//! Typed HTTPS client for the notes-server API. Response types are the
//! server's own models, so client and server cannot drift apart.

use std::path::{Path, PathBuf};
use std::time::Duration;

use notes_server::models::{
    Chapter, ChapterDetail, Course, CourseDetail, CreateChapter, CreateCourse, ImageDto, NoteDto,
    UpdateChapter, UpdateCourse, UpdateNote,
};
use reqwest::multipart::{Form, Part};
use reqwest::{Method, RequestBuilder, StatusCode};
use serde::de::DeserializeOwned;

/// Server page size cap for `GET /api/chapters/{id}`.
const PAGE: usize = 100;

#[derive(Debug, Clone)]
pub struct ApiError(pub String);

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

pub type ApiResult<T> = Result<T, ApiError>;

#[derive(Clone)]
pub struct Api {
    http: reqwest::Client,
    base: String,
    token: String,
}

impl Api {
    /// `base` may omit the scheme (`notes-pi.local` → `https://notes-pi.local`).
    /// `ca_cert` trusts an extra (e.g. self-signed) certificate; `insecure`
    /// skips verification entirely (development only).
    pub fn new(base: &str, token: &str, ca_cert: Option<&Path>, insecure: bool) -> ApiResult<Self> {
        let base = normalize_base(base);
        if base.is_empty() {
            return Err(ApiError("server URL is required".into()));
        }
        let mut builder = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(10))
            // Uploads of several large photos to a Pi on Wi-Fi can take a while.
            .timeout(Duration::from_secs(300))
            .user_agent(concat!("notes-admin/", env!("CARGO_PKG_VERSION")));
        if let Some(path) = ca_cert {
            let pem = std::fs::read(path)
                .map_err(|e| ApiError(format!("reading CA certificate {}: {e}", path.display())))?;
            let cert = reqwest::Certificate::from_pem(&pem)
                .map_err(|e| ApiError(format!("invalid CA certificate {}: {e}", path.display())))?;
            builder = builder.add_root_certificate(cert);
        }
        if insecure {
            builder = builder.danger_accept_invalid_certs(true);
        }
        let http = builder
            .build()
            .map_err(|e| ApiError(format!("building HTTP client: {e}")))?;
        Ok(Self { http, base, token: token.trim().to_owned() })
    }

    pub fn base(&self) -> &str {
        &self.base
    }

    fn request(&self, method: Method, path: &str) -> RequestBuilder {
        self.http
            .request(method, format!("{}{path}", self.base))
            .bearer_auth(&self.token)
    }

    async fn send(&self, req: RequestBuilder) -> ApiResult<reqwest::Response> {
        let res = req.send().await.map_err(describe_transport_error)?;
        if res.status().is_success() {
            return Ok(res);
        }
        let status = res.status();
        let body = res.text().await.unwrap_or_default();
        let message = serde_json::from_str::<serde_json::Value>(&body)
            .ok()
            .and_then(|v| v["error"].as_str().map(str::to_owned))
            .unwrap_or_else(|| body.trim().chars().take(200).collect());
        Err(ApiError(match status {
            StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => {
                format!("admin token rejected ({status})")
            }
            StatusCode::TOO_MANY_REQUESTS => {
                format!("rate limited, wait a few seconds and retry ({message})")
            }
            _ if message.is_empty() => format!("server returned {status}"),
            _ => format!("{message} ({})", status.as_u16()),
        }))
    }

    async fn json<T: DeserializeOwned>(&self, req: RequestBuilder) -> ApiResult<T> {
        self.send(req)
            .await?
            .json()
            .await
            .map_err(|e| ApiError(format!("unexpected response from server: {e}")))
    }

    // ---- Auth ----

    /// Verifies the server is reachable and the token is the admin token.
    pub async fn check_auth(&self) -> ApiResult<()> {
        self.send(self.request(Method::GET, "/api/auth/check")).await.map(drop)
    }

    // ---- Courses ----

    pub async fn courses(&self) -> ApiResult<Vec<Course>> {
        self.json(self.request(Method::GET, "/api/courses")).await
    }

    pub async fn course(&self, slug: &str) -> ApiResult<CourseDetail> {
        self.json(self.request(Method::GET, &format!("/api/courses/{slug}"))).await
    }

    pub async fn create_course(&self, req: &CreateCourse) -> ApiResult<Course> {
        self.json(self.request(Method::POST, "/api/courses").json(req)).await
    }

    pub async fn update_course(&self, slug: &str, req: &UpdateCourse) -> ApiResult<Course> {
        self.json(self.request(Method::PATCH, &format!("/api/courses/{slug}")).json(req))
            .await
    }

    pub async fn delete_course(&self, slug: &str) -> ApiResult<()> {
        self.send(self.request(Method::DELETE, &format!("/api/courses/{slug}")))
            .await
            .map(drop)
    }

    // ---- Chapters ----

    pub async fn create_chapter(&self, course_slug: &str, req: &CreateChapter) -> ApiResult<Chapter> {
        self.json(
            self.request(Method::POST, &format!("/api/courses/{course_slug}/chapters"))
                .json(req),
        )
        .await
    }

    pub async fn update_chapter(&self, id: &str, req: &UpdateChapter) -> ApiResult<Chapter> {
        self.json(self.request(Method::PATCH, &format!("/api/chapters/{id}")).json(req))
            .await
    }

    pub async fn delete_chapter(&self, id: &str) -> ApiResult<()> {
        self.send(self.request(Method::DELETE, &format!("/api/chapters/{id}")))
            .await
            .map(drop)
    }

    /// All notes of a chapter (follows pagination).
    pub async fn chapter_notes(&self, id: &str) -> ApiResult<Vec<NoteDto>> {
        let mut notes = Vec::new();
        loop {
            let page: ChapterDetail = self
                .json(self.request(
                    Method::GET,
                    &format!("/api/chapters/{id}?limit={PAGE}&offset={}", notes.len()),
                ))
                .await?;
            let got = page.notes.len();
            notes.extend(page.notes);
            if got < PAGE || notes.len() as i64 >= page.total_notes {
                return Ok(notes);
            }
        }
    }

    // ---- Notes ----

    pub async fn create_note(
        &self,
        chapter_id: &str,
        title: &str,
        body: &str,
        author_name: &str,
        files: &[PathBuf],
    ) -> ApiResult<NoteDto> {
        let mut form = Form::new()
            .text("title", title.to_owned())
            .text("body", body.to_owned())
            .text("author_name", author_name.to_owned());
        form = add_files(form, files).await?;
        self.json(
            self.request(Method::POST, &format!("/api/chapters/{chapter_id}/notes"))
                .multipart(form),
        )
        .await
    }

    pub async fn update_note(&self, id: &str, req: &UpdateNote) -> ApiResult<NoteDto> {
        self.json(self.request(Method::PATCH, &format!("/api/notes/{id}")).json(req))
            .await
    }

    pub async fn delete_note(&self, id: &str) -> ApiResult<()> {
        self.send(self.request(Method::DELETE, &format!("/api/notes/{id}")))
            .await
            .map(drop)
    }

    pub async fn add_images(&self, note_id: &str, files: &[PathBuf]) -> ApiResult<Vec<ImageDto>> {
        let form = add_files(Form::new(), files).await?;
        self.json(
            self.request(Method::POST, &format!("/api/notes/{note_id}/images"))
                .multipart(form),
        )
        .await
    }

    pub async fn delete_image(&self, note_id: &str, image_id: &str) -> ApiResult<()> {
        self.send(self.request(
            Method::DELETE,
            &format!("/api/notes/{note_id}/images/{image_id}"),
        ))
        .await
        .map(drop)
    }
}

async fn add_files(mut form: Form, files: &[PathBuf]) -> ApiResult<Form> {
    for path in files {
        let bytes = tokio::fs::read(path)
            .await
            .map_err(|e| ApiError(format!("reading {}: {e}", path.display())))?;
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "image".into());
        form = form.part("images", Part::bytes(bytes).file_name(name));
    }
    Ok(form)
}

fn normalize_base(base: &str) -> String {
    let base = base.trim().trim_end_matches('/');
    if base.is_empty() || base.contains("://") {
        base.to_owned()
    } else {
        format!("https://{base}")
    }
}

fn describe_transport_error(e: reqwest::Error) -> ApiError {
    // Walk the source chain: the useful part (e.g. "invalid peer certificate:
    // UnknownIssuer") is usually a few levels down.
    let mut msg = e.to_string();
    let mut source = std::error::Error::source(&e);
    while let Some(s) = source {
        msg.push_str(": ");
        msg.push_str(&s.to_string());
        source = s.source();
    }
    if msg.contains("UnknownIssuer") || msg.contains("certificate") {
        msg.push_str(" (self-signed certificate? pass --ca-cert <cert.pem>)");
    }
    ApiError(msg)
}

#[cfg(test)]
mod tests {
    use super::normalize_base;

    #[test]
    fn base_url_normalization() {
        assert_eq!(normalize_base("notes-pi.local"), "https://notes-pi.local");
        assert_eq!(normalize_base(" https://x.y/ "), "https://x.y");
        assert_eq!(normalize_base("http://127.0.0.1:8080"), "http://127.0.0.1:8080");
        assert_eq!(normalize_base(""), "");
    }
}
