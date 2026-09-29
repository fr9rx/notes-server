mod chapters;
mod courses;
mod notes;
mod stats;

use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::extract::{ConnectInfo, DefaultBodyLimit};
use axum::http::{HeaderValue, Method, Request, Response, StatusCode, header};
use axum::routing::{delete, get, post};
use tower::ServiceBuilder;
use tower_governor::governor::GovernorConfigBuilder;
use tower_governor::key_extractor::KeyExtractor;
use tower_governor::{GovernorError, GovernorLayer};
use tower_http::cors::{AllowOrigin, CorsLayer};
use tower_http::services::ServeDir;
use tower_http::set_header::SetResponseHeaderLayer;
use tower_http::trace::TraceLayer;

use crate::AppState;
use crate::auth::RequireAdmin;
use crate::error::{AppError, AppResult};

pub use notes::{MAX_IMAGE_BYTES, MAX_IMAGES_PER_REQUEST};

/// Cap for a whole multipart upload request (10 images × 10 MB + form fields, with headroom).
pub const MAX_UPLOAD_REQUEST_BYTES: usize = 60 * 1024 * 1024;

/// Builds the application. Must be called from within a Tokio runtime (it
/// spawns the rate limiter's cleanup task). Serve it with
/// `into_make_service_with_connect_info::<SocketAddr>()` so the per-IP upload
/// limiter can see client addresses.
pub fn router(state: AppState) -> Router {
    let settings = &state.settings;

    let governor = Arc::new(
        GovernorConfigBuilder::default()
            .per_second(settings.upload_refill_secs)
            .burst_size(settings.upload_burst)
            .key_extractor(ClientIp)
            .finish()
            .expect("rate limit settings are non-zero"),
    );
    let limiter = governor.limiter().clone();
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_secs(60));
        loop {
            tick.tick().await;
            limiter.retain_recent();
        }
    });

    let upload_limit = DefaultBodyLimit::max(MAX_UPLOAD_REQUEST_BYTES);

    // The only write anyone can do without the admin token, so it is rate limited.
    let public_upload = Router::new()
        .route(
            "/api/chapters/{id}/notes",
            post(notes::create).layer(upload_limit),
        )
        .route_layer(GovernorLayer::new(governor));

    let api = Router::new()
        .route("/api/auth/check", get(auth_check))
        .route("/api/courses", get(courses::list).post(courses::create))
        .route(
            "/api/courses/{slug}",
            get(courses::get).patch(courses::update).delete(courses::delete),
        )
        .route("/api/courses/{slug}/chapters", post(chapters::create))
        .route(
            "/api/chapters/{id}",
            get(chapters::get).patch(chapters::update).delete(chapters::delete),
        )
        .route(
            "/api/notes/{id}",
            get(notes::get).patch(notes::update).delete(notes::delete),
        )
        .route(
            "/api/notes/{id}/images",
            post(notes::add_images).layer(upload_limit),
        )
        .route("/api/notes/{id}/images/{image_id}", delete(notes::delete_image))
        .merge(public_upload);

    // Keys are never reused, so files can be cached forever. Only successful
    // responses get the long cache header; a 404 must not stick.
    let files = ServiceBuilder::new()
        .layer(SetResponseHeaderLayer::if_not_present(
            header::CACHE_CONTROL,
            |res: &Response<_>| {
                res.status()
                    .is_success()
                    .then(|| HeaderValue::from_static("public, max-age=31536000, immutable"))
            },
        ))
        .layer(SetResponseHeaderLayer::overriding(
            header::X_CONTENT_TYPE_OPTIONS,
            HeaderValue::from_static("nosniff"),
        ))
        .service(ServeDir::new(state.storage.root()));

    let metrics = state.metrics.clone();
    Router::new()
        .merge(api)
        .route("/api/stats", get(stats::get))
        .route("/health", get(|| async { "ok" }))
        .nest_service("/files", files)
        // Everything else: the React app (static assets + client-side routes).
        .fallback(crate::web::serve)
        .layer(axum::middleware::from_fn(move |req: axum::extract::Request, next: axum::middleware::Next| {
            // Count real traffic for the LED graph: the API and images, but not the
            // website's own stats polling or its static assets.
            let path = req.uri().path();
            if (path.starts_with("/api/") && path != "/api/stats") || path.starts_with("/files/") {
                metrics.record_request();
            }
            next.run(req)
        }))
        .layer(TraceLayer::new_for_http())
        .layer(cors(settings.cors_origin.as_deref()))
        .layer(SetResponseHeaderLayer::if_not_present(
            header::STRICT_TRANSPORT_SECURITY,
            HeaderValue::from_static("max-age=31536000"),
        ))
        .with_state(state)
}

/// Rate-limit key: the client's IP. Connections from loopback come through
/// the local Cloudflare Tunnel (`cloudflared`), whose `CF-Connecting-IP`
/// header carries the real visitor. The header is ignored from anyone else,
/// so clients connecting directly can't forge it to dodge their limit.
#[derive(Debug, Clone, Copy)]
struct ClientIp;

impl KeyExtractor for ClientIp {
    type Key = IpAddr;

    fn extract<T>(&self, req: &Request<T>) -> Result<IpAddr, GovernorError> {
        let peer = req
            .extensions()
            .get::<ConnectInfo<SocketAddr>>()
            .map(|c| c.ip().to_canonical())
            .ok_or(GovernorError::UnableToExtractKey)?;
        if peer.is_loopback()
            && let Some(ip) = req
                .headers()
                .get("cf-connecting-ip")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.trim().parse().ok())
        {
            return Ok(ip);
        }
        Ok(peer)
    }
}

/// 204 if the bearer token is the admin token (401/403 otherwise). Lets
/// clients such as `notes-admin` verify credentials without side effects.
async fn auth_check(_: RequireAdmin) -> StatusCode {
    StatusCode::NO_CONTENT
}

fn cors(origin: Option<&str>) -> CorsLayer {
    let layer = CorsLayer::new()
        .allow_methods([Method::GET, Method::POST, Method::PATCH, Method::DELETE])
        .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE])
        .max_age(Duration::from_secs(3600));
    match origin {
        // Validated in Config::from_env.
        Some(origin) => layer.allow_origin(
            HeaderValue::from_str(origin).expect("CORS_ORIGIN is a valid header value"),
        ),
        None => layer.allow_origin(AllowOrigin::any()),
    }
}

// ---- Shared validation -----------------------------------------------------

const MAX_TITLE_CHARS: usize = 200;
const MAX_AUTHOR_CHARS: usize = 100;
const MAX_DESCRIPTION_BYTES: usize = 10 * 1024;
pub const MAX_BODY_BYTES: usize = 100 * 1024;

/// Trims and checks a required short text field (titles, names).
fn required_text(field: &str, value: &str) -> AppResult<String> {
    let value = value.trim();
    if value.is_empty() {
        return Err(AppError::BadRequest(format!("`{field}` is required")));
    }
    if value.chars().count() > MAX_TITLE_CHARS {
        return Err(AppError::BadRequest(format!(
            "`{field}` must be at most {MAX_TITLE_CHARS} characters"
        )));
    }
    Ok(value.to_owned())
}

/// Trims the author name; empty means "none".
fn author_name(value: &str) -> AppResult<String> {
    let value = value.trim();
    if value.chars().count() > MAX_AUTHOR_CHARS {
        return Err(AppError::BadRequest(format!(
            "`author_name` must be at most {MAX_AUTHOR_CHARS} characters"
        )));
    }
    Ok(value.to_owned())
}

fn note_body(value: &str) -> AppResult<()> {
    if value.len() > MAX_BODY_BYTES {
        return Err(AppError::BadRequest(format!(
            "`body` must be at most {} KB",
            MAX_BODY_BYTES / 1024
        )));
    }
    Ok(())
}

fn description(value: &str) -> AppResult<()> {
    if value.len() > MAX_DESCRIPTION_BYTES {
        return Err(AppError::BadRequest(format!(
            "`description` must be at most {} KB",
            MAX_DESCRIPTION_BYTES / 1024
        )));
    }
    Ok(())
}

fn slug(value: &str) -> AppResult<()> {
    let ok = (1..=64).contains(&value.len())
        && value
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        && !value.starts_with('-')
        && !value.ends_with('-');
    if ok {
        Ok(())
    } else {
        Err(AppError::BadRequest(
            "`slug` must be 1-64 characters of a-z, 0-9 and '-', not starting or ending with '-'"
                .into(),
        ))
    }
}

fn position(value: Option<i64>) -> AppResult<()> {
    match value {
        Some(p) if p < 0 => Err(AppError::BadRequest("`position` must be >= 0".into())),
        _ => Ok(()),
    }
}
