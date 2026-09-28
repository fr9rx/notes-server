//! Serves the React frontend (`frontend/`, built by Vite into `frontend/dist`).
//!
//! In release builds the files are embedded in the binary, so the board still
//! gets a single file to deploy; debug builds read them from disk, so a
//! `vite build --watch` shows up without recompiling Rust.
//!
//! - Hashed assets (`/assets/*`) are cached forever; `index.html` is revalidated
//!   with its ETag every time.
//! - Vite emits `.br` and `.gz` next to each asset; the smallest variant the
//!   client accepts is served as-is, so the board never compresses anything.
//! - Every other path that isn't `/api/*` or `/files/*` gets `index.html`, so
//!   the client-side router can handle deep links like `/course/math-101`.

use axum::body::Body;
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode, Uri, header};
use axum::response::{IntoResponse, Response};
use rust_embed::RustEmbed;

use crate::error::AppError;

#[derive(RustEmbed)]
#[folder = "frontend/dist"]
#[exclude = "*.br"]
#[exclude = "*.gz"]
struct Assets;

/// The precompressed variants, looked up by name.
#[derive(RustEmbed)]
#[folder = "frontend/dist"]
#[include = "*.br"]
#[include = "*.gz"]
struct Compressed;

const CSP: &str = "default-src 'self'; img-src 'self' blob: data:; style-src 'self' 'unsafe-inline'; \
                   script-src 'self'; connect-src 'self'; font-src 'self'; object-src 'none'; \
                   base-uri 'none'; frame-ancestors 'none'; form-action 'self'";

/// Router fallback: static files and the SPA shell.
pub async fn serve(method: Method, uri: Uri, headers: HeaderMap) -> Response {
    let path = uri.path();
    if path.starts_with("/api/") || path == "/api" {
        return AppError::NotFound("endpoint").into_response();
    }
    if method != Method::GET && method != Method::HEAD {
        return StatusCode::METHOD_NOT_ALLOWED.into_response();
    }

    let relative = path.trim_start_matches('/');
    let (name, file, is_shell) = match Assets::get(relative).filter(|_| !relative.is_empty()) {
        Some(file) => (relative.to_owned(), file, relative == "index.html"),
        None => match Assets::get("index.html") {
            // Unknown asset paths are real 404s; anything else is a client-side route.
            Some(_) if relative.starts_with("assets/") => {
                return StatusCode::NOT_FOUND.into_response();
            }
            Some(file) => ("index.html".to_owned(), file, true),
            None => return (StatusCode::NOT_FOUND, "frontend not built").into_response(),
        },
    };

    let etag = format!("\"{}\"", hex(&file.metadata.sha256_hash()[..16]));
    let mut res = if headers
        .get(header::IF_NONE_MATCH)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.split(',').any(|t| t.trim() == etag || t.trim() == "*"))
    {
        StatusCode::NOT_MODIFIED.into_response()
    } else {
        let (body, encoding) = pick_encoding(&name, &headers)
            .unwrap_or_else(|| (file.data.clone().into_owned(), None));
        let mut res = Response::new(if method == Method::HEAD { Body::empty() } else { Body::from(body) });
        if let Some(enc) = encoding {
            res.headers_mut()
                .insert(header::CONTENT_ENCODING, HeaderValue::from_static(enc));
        }
        res
    };

    let h = res.headers_mut();
    h.insert(header::ETAG, HeaderValue::from_str(&etag).expect("hex etag"));
    h.insert(header::VARY, HeaderValue::from_static("Accept-Encoding"));
    if let Ok(mime) = HeaderValue::from_str(file.metadata.mimetype()) {
        h.insert(header::CONTENT_TYPE, mime);
    }
    h.insert(header::X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
    if is_shell {
        h.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
        h.insert(header::CONTENT_SECURITY_POLICY, HeaderValue::from_static(CSP));
        h.insert(header::X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
        h.insert(header::REFERRER_POLICY, HeaderValue::from_static("same-origin"));
    } else if name.starts_with("assets/") {
        h.insert(
            header::CACHE_CONTROL,
            HeaderValue::from_static("public, max-age=31536000, immutable"),
        );
    } else {
        h.insert(header::CACHE_CONTROL, HeaderValue::from_static("public, max-age=3600"));
    }
    res
}

/// Brotli, then gzip, if the client accepts it and Vite produced it.
fn pick_encoding(name: &str, headers: &HeaderMap) -> Option<(Vec<u8>, Option<&'static str>)> {
    let accept = headers
        .get(header::ACCEPT_ENCODING)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    let accepts = |enc: &str| {
        accept.split(',').any(|part| {
            let mut it = part.split(';');
            let token = it.next().unwrap_or_default().trim();
            let q_zero = it.any(|p| p.trim().replace(' ', "") == "q=0");
            token.eq_ignore_ascii_case(enc) && !q_zero
        })
    };
    for (ext, enc) in [("br", "br"), ("gz", "gzip")] {
        if accepts(enc)
            && let Some(file) = Compressed::get(&format!("{name}.{ext}"))
        {
            return Some((file.data.into_owned(), Some(enc)));
        }
    }
    None
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
