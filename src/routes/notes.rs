use std::time::Instant;

use axum::Json;
use axum::extract::multipart::Field;
use axum::extract::{Multipart, Path, State};
use axum::http::StatusCode;
use tokio::task::JoinSet;
use uuid::Uuid;

use crate::AppState;
use crate::auth::RequireAdmin;
use crate::db;
use crate::error::{AppError, AppResult, is_fk_violation};
use crate::imaging::{self, ImageError, Processed};
use crate::models::{ImageDto, ImageRow, NoteDto, UpdateNote};
use crate::storage::{image_key, thumb_key};

pub const MAX_IMAGES_PER_REQUEST: usize = 10;
pub const MAX_IMAGE_BYTES: usize = 10 * 1024 * 1024;
/// Cap for short text form fields (title, author_name) before validation.
const MAX_SHORT_FIELD_BYTES: usize = 4 * 1024;

// ---- Public ----------------------------------------------------------------

/// Anyone may upload a note (title, body, author_name, 1-10 `images`).
pub async fn create(
    State(state): State<AppState>,
    Path(chapter_id): Path<String>,
    multipart: Multipart,
) -> AppResult<(StatusCode, Json<NoteDto>)> {
    ensure_disk_space(&state)?;
    db::chapter_by_id(&state.db, &chapter_id)
        .await?
        .ok_or(AppError::NotFound("chapter"))?;

    let form = read_form(multipart, true).await?;
    let title = super::required_text("title", form.title.as_deref().unwrap_or_default())?;
    let body = form.body.unwrap_or_default();
    super::note_body(&body)?;
    let author = super::author_name(form.author_name.as_deref().unwrap_or_default())?;
    let author = (!author.is_empty()).then_some(author);
    if form.files.is_empty() {
        return Err(AppError::BadRequest(
            "at least one image is required (form field `images`)".into(),
        ));
    }

    let processed = process_images(&state, form.files).await?;
    let note_id = Uuid::now_v7().to_string();
    let (rows, _) = store_images(&state, &note_id, processed).await?;

    let saved = async {
        let mut tx = state.db.begin().await?;
        let note =
            db::insert_note(&mut *tx, &note_id, &chapter_id, &title, &body, author.as_deref()).await?;
        let mut images = Vec::with_capacity(rows.len());
        for row in &rows {
            images.push(db::insert_image(&mut *tx, row).await?);
        }
        tx.commit().await?;
        Ok::<_, sqlx::Error>((note, images))
    }
    .await;

    match saved {
        Ok((note, images)) => {
            tracing::info!(note = %note.id, chapter = %chapter_id, images = images.len(), "note uploaded");
            state.metrics.record_uploads(images.len());
            Ok((StatusCode::CREATED, Json(NoteDto::new(note, images, &state.storage))))
        }
        Err(e) => {
            state.storage.delete_notes_logged(&[note_id]).await;
            // The chapter was deleted while we were processing.
            Err(if is_fk_violation(&e) { AppError::NotFound("chapter") } else { e.into() })
        }
    }
}

pub async fn get(State(state): State<AppState>, Path(id): Path<String>) -> AppResult<Json<NoteDto>> {
    let note = db::note_by_id(&state.db, &id)
        .await?
        .ok_or(AppError::NotFound("note"))?;
    let images = db::images_for_note(&state.db, &id).await?;
    Ok(Json(NoteDto::new(note, images, &state.storage)))
}

// ---- Admin -----------------------------------------------------------------

pub async fn update(
    _: RequireAdmin,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(mut req): Json<UpdateNote>,
) -> AppResult<Json<NoteDto>> {
    if let Some(title) = &req.title {
        req.title = Some(super::required_text("title", title)?);
    }
    if let Some(body) = &req.body {
        super::note_body(body)?;
    }
    if let Some(author) = &req.author_name {
        req.author_name = Some(super::author_name(author)?);
    }
    if let Some(chapter_id) = &req.chapter_id
        && db::chapter_by_id(&state.db, chapter_id).await?.is_none()
    {
        return Err(AppError::BadRequest(format!(
            "`chapter_id` {chapter_id} does not exist"
        )));
    }

    let note = match db::update_note(&state.db, &id, &req).await {
        Ok(Some(note)) => note,
        Ok(None) => return Err(AppError::NotFound("note")),
        Err(e) if is_fk_violation(&e) => {
            return Err(AppError::BadRequest("`chapter_id` does not exist".into()));
        }
        Err(e) => return Err(e.into()),
    };
    let images = db::images_for_note(&state.db, &id).await?;
    Ok(Json(NoteDto::new(note, images, &state.storage)))
}

pub async fn delete(
    _: RequireAdmin,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<StatusCode> {
    if db::delete_note(&state.db, &id).await? == 0 {
        return Err(AppError::NotFound("note"));
    }
    state.storage.delete_notes_logged(std::slice::from_ref(&id)).await;
    tracing::info!(note = %id, "deleted note");
    Ok(StatusCode::NO_CONTENT)
}

/// Appends images to an existing note. Admin-only: it modifies someone
/// else's upload.
pub async fn add_images(
    _: RequireAdmin,
    State(state): State<AppState>,
    Path(note_id): Path<String>,
    multipart: Multipart,
) -> AppResult<(StatusCode, Json<Vec<ImageDto>>)> {
    ensure_disk_space(&state)?;
    db::note_by_id(&state.db, &note_id)
        .await?
        .ok_or(AppError::NotFound("note"))?;

    let form = read_form(multipart, false).await?;
    if form.files.is_empty() {
        return Err(AppError::BadRequest(
            "at least one image is required (form field `images`)".into(),
        ));
    }
    let processed = process_images(&state, form.files).await?;
    let (rows, written) = store_images(&state, &note_id, processed).await?;

    let saved = async {
        let mut tx = state.db.begin().await?;
        let base = db::next_image_position(&mut *tx, &note_id).await?;
        let mut images = Vec::with_capacity(rows.len());
        for mut row in rows {
            row.position += base;
            images.push(db::insert_image(&mut *tx, &row).await?);
        }
        db::touch_note(&mut *tx, &note_id).await?;
        tx.commit().await?;
        Ok::<_, sqlx::Error>(images)
    }
    .await;

    match saved {
        Ok(images) => {
            state.metrics.record_uploads(images.len());
            Ok((
            StatusCode::CREATED,
            Json(images.into_iter().map(|r| ImageDto::new(r, &state.storage)).collect()),
        ))
        }
        Err(e) => {
            state.storage.delete_keys_logged(&written).await;
            Err(if is_fk_violation(&e) { AppError::NotFound("note") } else { e.into() })
        }
    }
}

pub async fn delete_image(
    _: RequireAdmin,
    State(state): State<AppState>,
    Path((note_id, image_id)): Path<(String, String)>,
) -> AppResult<StatusCode> {
    let (key, thumb) = db::delete_image(&state.db, &note_id, &image_id)
        .await?
        .ok_or(AppError::NotFound("image"))?;
    db::touch_note(&state.db, &note_id).await?;
    state.storage.delete_keys_logged(&[key, thumb]).await;
    Ok(StatusCode::NO_CONTENT)
}

// ---- Upload plumbing -------------------------------------------------------

/// Reads a multipart body holding exactly one image (field `image`) and
/// resizes it. Used for course covers.
pub(super) async fn process_single_image(state: &AppState, multipart: Multipart) -> AppResult<Processed> {
    ensure_disk_space(state)?;
    let form = read_form(multipart, false).await?;
    if form.files.len() != 1 {
        return Err(AppError::BadRequest("send exactly one image (form field `image`)".into()));
    }
    let (_, processed) = process_images(state, form.files)
        .await?
        .pop()
        .expect("one image in, one out");
    Ok(processed)
}

struct UploadedFile {
    filename: Option<String>,
    bytes: Vec<u8>,
}

#[derive(Default)]
struct UploadForm {
    title: Option<String>,
    body: Option<String>,
    author_name: Option<String>,
    files: Vec<UploadedFile>,
}

/// Streams the multipart body, enforcing per-field size caps as it goes.
/// `accept_text = false` only allows `images` fields.
async fn read_form(mut multipart: Multipart, accept_text: bool) -> AppResult<UploadForm> {
    let mut form = UploadForm::default();
    while let Some(mut field) = multipart.next_field().await? {
        let name = field.name().unwrap_or_default().to_owned();
        match name.as_str() {
            "images" | "images[]" | "image" => {
                let filename = field.file_name().map(clean_filename);
                let bytes = read_capped(&mut field, MAX_IMAGE_BYTES).await?.ok_or_else(|| {
                    AppError::PayloadTooLarge(format!(
                        "each image must be at most {} MB",
                        MAX_IMAGE_BYTES / (1024 * 1024)
                    ))
                })?;
                // Browsers send an empty part for an untouched <input type=file>.
                if bytes.is_empty() {
                    continue;
                }
                if form.files.len() == MAX_IMAGES_PER_REQUEST {
                    return Err(AppError::PayloadTooLarge(format!(
                        "at most {MAX_IMAGES_PER_REQUEST} images per request"
                    )));
                }
                form.files.push(UploadedFile { filename, bytes });
            }
            "title" | "body" | "author_name" if accept_text => {
                let cap = if name == "body" { super::MAX_BODY_BYTES } else { MAX_SHORT_FIELD_BYTES };
                let bytes = read_capped(&mut field, cap)
                    .await?
                    .ok_or_else(|| AppError::BadRequest(format!("`{name}` is too long")))?;
                let text = String::from_utf8(bytes)
                    .map_err(|_| AppError::BadRequest(format!("`{name}` must be valid UTF-8")))?;
                match name.as_str() {
                    "title" => form.title = Some(text),
                    "body" => form.body = Some(text),
                    _ => form.author_name = Some(text),
                }
            }
            other => {
                return Err(AppError::BadRequest(format!("unexpected form field `{other}`")));
            }
        }
    }
    Ok(form)
}

/// `Ok(None)` if the field exceeds `cap` bytes.
async fn read_capped(field: &mut Field<'_>, cap: usize) -> AppResult<Option<Vec<u8>>> {
    let mut buf = Vec::new();
    while let Some(chunk) = field.chunk().await? {
        if buf.len() + chunk.len() > cap {
            return Ok(None);
        }
        buf.extend_from_slice(&chunk);
    }
    Ok(Some(buf))
}

/// Client filenames are display metadata only (never used as paths): keep
/// the last path component, drop control characters, bound the length.
fn clean_filename(name: &str) -> String {
    let base = name.rsplit(['/', '\\']).next().unwrap_or(name);
    base.chars().filter(|c| !c.is_control()).take(200).collect()
}

/// Rejects uploads when the disk is nearly full.
fn ensure_disk_space(state: &AppState) -> AppResult<()> {
    let free = state.storage.available_bytes()?;
    if free < state.settings.min_free_disk_bytes {
        tracing::warn!(free_bytes = free, "refusing upload: low disk space");
        return Err(AppError::InsufficientStorage);
    }
    Ok(())
}

/// Decodes and resizes all files, in parallel up to `IMAGE_WORKERS` (shared
/// across all requests). Fails the whole batch if any image is bad, before
/// anything is written. Output order matches input order.
async fn process_images(
    state: &AppState,
    files: Vec<UploadedFile>,
) -> AppResult<Vec<(Option<String>, Processed)>> {
    let count = files.len();
    let mut tasks = JoinSet::new();
    for (index, file) in files.into_iter().enumerate() {
        let permits = state.image_permits.clone();
        tasks.spawn(async move {
            let _permit = permits.acquire_owned().await.map_err(AppError::internal)?;
            let UploadedFile { filename, bytes } = file;
            let started = Instant::now();
            let size = bytes.len();
            let processed = tokio::task::spawn_blocking(move || imaging::process(&bytes))
                .await
                .map_err(AppError::internal)?
                .map_err(|e| label_image_error(e, index, filename.as_deref()))?;
            tracing::debug!(
                input_bytes = size,
                width = processed.main.width,
                height = processed.main.height,
                elapsed_ms = started.elapsed().as_millis() as u64,
                "processed image"
            );
            Ok::<_, AppError>((index, filename, processed))
        });
    }

    let mut out = Vec::with_capacity(count);
    while let Some(joined) = tasks.join_next().await {
        // Returning early drops the JoinSet, which aborts the remaining tasks.
        out.push(joined.map_err(AppError::internal)??);
    }
    out.sort_unstable_by_key(|(index, ..)| *index);
    Ok(out.into_iter().map(|(_, name, p)| (name, p)).collect())
}

fn label_image_error(e: ImageError, index: usize, filename: Option<&str>) -> AppError {
    let label = match filename {
        Some(name) if !name.is_empty() => format!("image {} ({name})", index + 1),
        _ => format!("image {}", index + 1),
    };
    match AppError::from(e) {
        AppError::UnsupportedMedia(msg) => AppError::UnsupportedMedia(format!("{label}: {msg}")),
        AppError::PayloadTooLarge(msg) => AppError::PayloadTooLarge(format!("{label}: {msg}")),
        other => other,
    }
}

/// Writes main + thumbnail files and returns the rows to insert (positions
/// 0..n, relative) plus every key written, for cleanup if the DB insert fails.
async fn store_images(
    state: &AppState,
    note_id: &str,
    processed: Vec<(Option<String>, Processed)>,
) -> AppResult<(Vec<ImageRow>, Vec<String>)> {
    let mut rows = Vec::with_capacity(processed.len());
    let mut written = Vec::with_capacity(processed.len() * 2);

    for (position, (filename, p)) in processed.into_iter().enumerate() {
        let image_id = Uuid::now_v7().to_string();
        let key = image_key(note_id, &image_id);
        let tkey = thumb_key(note_id, &image_id);

        for (k, bytes) in [(&key, &p.main.bytes), (&tkey, &p.thumb.bytes)] {
            if let Err(e) = state.storage.put(k, bytes).await {
                state.storage.delete_keys_logged(&written).await;
                return Err(e.into());
            }
            written.push(k.clone());
        }

        rows.push(ImageRow {
            id: image_id,
            note_id: note_id.to_owned(),
            position: position as i64,
            original_filename: filename,
            storage_key: key,
            width: p.main.width.into(),
            height: p.main.height.into(),
            size_bytes: p.main.bytes.len() as i64,
            thumb_key: tkey,
            thumb_width: p.thumb.width.into(),
            thumb_height: p.thumb.height.into(),
            created_at: String::new(), // set by the database
        });
    }
    Ok((rows, written))
}
