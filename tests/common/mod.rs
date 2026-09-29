#![allow(dead_code)] // each test binary uses a different subset

use std::net::SocketAddr;
use std::path::PathBuf;

use axum::Router;
use axum::body::{Body, Bytes};
use axum::extract::ConnectInfo;
use axum::http::{HeaderMap, Method, Request, StatusCode, header};
use http_body_util::BodyExt;
use notes_server::config::AppSettings;
use notes_server::{AppState, db};
use serde_json::Value;
use sqlx::SqlitePool;
use tempfile::TempDir;
use tower::ServiceExt;

pub const ADMIN: &str = "test-admin-token-0123456789abcdef";
pub const BASE_URL: &str = "https://notes.test";

pub struct TestApp {
    pub app: Router,
    pub pool: SqlitePool,
    pub dir: TempDir,
}

impl TestApp {
    pub async fn new() -> Self {
        Self::with(|_| {}).await
    }

    pub async fn with(tweak: impl FnOnce(&mut AppSettings)) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let url = format!("sqlite://{}", db_path.display().to_string().replace('\\', "/"));
        let pool = db::connect(&url).await.unwrap();
        db::migrate(&pool).await.unwrap();

        let mut settings = AppSettings::new(ADMIN, BASE_URL);
        settings.min_free_disk_bytes = 0;
        settings.upload_burst = 1000;
        tweak(&mut settings);

        let state = AppState::new(pool.clone(), dir.path().join("uploads"), settings).unwrap();
        Self { app: notes_server::app(state), pool, dir }
    }

    pub fn uploads(&self) -> PathBuf {
        self.dir.path().join("uploads")
    }

    /// Path on disk for an image URL returned by the API.
    pub fn file_for_url(&self, url: &str) -> PathBuf {
        let key = url
            .strip_prefix(&format!("{BASE_URL}/files/"))
            .expect("url under /files");
        self.uploads().join(key)
    }

    pub async fn call(&self, mut req: Request<Body>) -> Resp {
        // What `into_make_service_with_connect_info` provides in production;
        // the per-IP rate limiter needs it.
        req.extensions_mut()
            .insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 40000))));
        let res = self.app.clone().oneshot(req).await.unwrap();
        let status = res.status();
        let headers = res.headers().clone();
        let body = res.into_body().collect().await.unwrap().to_bytes();
        Resp { status, headers, body }
    }

    pub async fn get(&self, uri: &str) -> Resp {
        self.call(Request::get(uri).body(Body::empty()).unwrap()).await
    }

    pub async fn json(&self, method: Method, uri: &str, token: Option<&str>, body: Value) -> Resp {
        let mut req = Request::builder()
            .method(method)
            .uri(uri)
            .header(header::CONTENT_TYPE, "application/json");
        if let Some(t) = token {
            req = req.header(header::AUTHORIZATION, format!("Bearer {t}"));
        }
        self.call(req.body(Body::from(body.to_string())).unwrap()).await
    }

    pub async fn delete(&self, uri: &str, token: Option<&str>) -> Resp {
        let mut req = Request::delete(uri);
        if let Some(t) = token {
            req = req.header(header::AUTHORIZATION, format!("Bearer {t}"));
        }
        self.call(req.body(Body::empty()).unwrap()).await
    }

    /// Admin-creates a course and a chapter, returning `(slug, chapter_id)`.
    pub async fn course_with_chapter(&self, slug: &str) -> (String, String) {
        let r = self
            .json(Method::POST, "/api/courses", Some(ADMIN), serde_json::json!({ "slug": slug, "name": slug }))
            .await;
        assert_eq!(r.status, StatusCode::CREATED, "{}", r.text());
        let r = self
            .json(
                Method::POST,
                &format!("/api/courses/{slug}/chapters"),
                Some(ADMIN),
                serde_json::json!({ "title": "Chapter 1" }),
            )
            .await;
        assert_eq!(r.status, StatusCode::CREATED, "{}", r.text());
        (slug.to_owned(), r.json()["id"].as_str().unwrap().to_owned())
    }

    /// Public upload of a note with the given images.
    pub async fn upload_note(&self, chapter_id: &str, title: &str, images: &[Vec<u8>]) -> Resp {
        let mut form = Form::new().text("title", title);
        for (i, img) in images.iter().enumerate() {
            form = form.file("images", &format!("photo{i}.jpg"), img);
        }
        self.call(form.request(&format!("/api/chapters/{chapter_id}/notes"), None))
            .await
    }
}

pub struct Resp {
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub body: Bytes,
}

impl Resp {
    pub fn json(&self) -> Value {
        serde_json::from_slice(&self.body)
            .unwrap_or_else(|e| panic!("invalid JSON ({e}): {}", self.text()))
    }

    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }
}

/// Minimal multipart/form-data builder.
pub struct Form {
    boundary: String,
    body: Vec<u8>,
}

impl Form {
    pub fn new() -> Self {
        Self { boundary: "----notes-test-boundary-7f3a9c".into(), body: Vec::new() }
    }

    pub fn text(mut self, name: &str, value: &str) -> Self {
        self.body.extend_from_slice(
            format!(
                "--{}\r\nContent-Disposition: form-data; name=\"{name}\"\r\n\r\n{value}\r\n",
                self.boundary
            )
            .as_bytes(),
        );
        self
    }

    pub fn file(mut self, name: &str, filename: &str, bytes: &[u8]) -> Self {
        self.body.extend_from_slice(
            format!(
                "--{}\r\nContent-Disposition: form-data; name=\"{name}\"; filename=\"{filename}\"\r\n\
                 Content-Type: application/octet-stream\r\n\r\n",
                self.boundary
            )
            .as_bytes(),
        );
        self.body.extend_from_slice(bytes);
        self.body.extend_from_slice(b"\r\n");
        self
    }

    pub fn request(self, uri: &str, token: Option<&str>) -> Request<Body> {
        self.request_with(Method::POST, uri, token)
    }

    pub fn request_with(mut self, method: Method, uri: &str, token: Option<&str>) -> Request<Body> {
        self.body
            .extend_from_slice(format!("--{}--\r\n", self.boundary).as_bytes());
        let mut req = Request::builder().method(method).uri(uri).header(
            header::CONTENT_TYPE,
            format!("multipart/form-data; boundary={}", self.boundary),
        );
        if let Some(t) = token {
            req = req.header(header::AUTHORIZATION, format!("Bearer {t}"));
        }
        req.body(Body::from(self.body)).unwrap()
    }
}

/// A gradient JPEG of the given size.
pub fn jpeg(width: u16, height: u16) -> Vec<u8> {
    let mut rgb = Vec::with_capacity(width as usize * height as usize * 3);
    for y in 0..height {
        for x in 0..width {
            rgb.extend_from_slice(&[(x % 256) as u8, (y % 256) as u8, 128]);
        }
    }
    let mut out = Vec::new();
    jpeg_encoder::Encoder::new(&mut out, 90)
        .encode(&rgb, width, height, jpeg_encoder::ColorType::Rgb)
        .unwrap();
    out
}

pub fn small_jpeg() -> Vec<u8> {
    jpeg(64, 48)
}
