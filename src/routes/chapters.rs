use std::collections::HashMap;

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use uuid::Uuid;

use crate::AppState;
use crate::auth::RequireAdmin;
use crate::db;
use crate::error::{AppError, AppResult};
use crate::models::{Chapter, ChapterDetail, CreateChapter, ImageRow, NoteDto, Page, UpdateChapter};

const DEFAULT_PAGE_SIZE: i64 = 50;
const MAX_PAGE_SIZE: i64 = 100;

pub async fn create(
    _: RequireAdmin,
    State(state): State<AppState>,
    Path(slug): Path<String>,
    Json(req): Json<CreateChapter>,
) -> AppResult<(StatusCode, Json<Chapter>)> {
    let title = super::required_text("title", &req.title)?;
    super::position(req.position)?;

    let course = db::course_by_slug(&state.db, &slug)
        .await?
        .ok_or(AppError::NotFound("course"))?;
    let id = Uuid::now_v7().to_string();
    let chapter = db::insert_chapter(&state.db, &id, &course.id, &title, req.position).await?;
    Ok((StatusCode::CREATED, Json(chapter)))
}

/// The chapter with one page of its notes (oldest first) and their images.
pub async fn get(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(page): Query<Page>,
) -> AppResult<Json<ChapterDetail>> {
    let limit = page.limit.unwrap_or(DEFAULT_PAGE_SIZE).clamp(1, MAX_PAGE_SIZE);
    let offset = page.offset.unwrap_or(0).max(0);

    // One read transaction = one consistent snapshot across the queries.
    let mut tx = state.db.begin().await?;
    let chapter = db::chapter_by_id(&mut *tx, &id)
        .await?
        .ok_or(AppError::NotFound("chapter"))?;
    let total_notes = db::count_notes(&mut *tx, &id).await?;
    let notes = db::notes_page(&mut *tx, &id, limit, offset).await?;
    let images = db::images_for_notes_page(&mut *tx, &id, limit, offset).await?;
    tx.commit().await?;

    let mut by_note: HashMap<String, Vec<ImageRow>> = HashMap::new();
    for image in images {
        by_note.entry(image.note_id.clone()).or_default().push(image);
    }
    let notes = notes
        .into_iter()
        .map(|note| {
            let images = by_note.remove(&note.id).unwrap_or_default();
            NoteDto::new(note, images, &state.storage)
        })
        .collect();

    Ok(Json(ChapterDetail { chapter, notes, total_notes, limit, offset }))
}

pub async fn update(
    _: RequireAdmin,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(mut req): Json<UpdateChapter>,
) -> AppResult<Json<Chapter>> {
    if let Some(title) = &req.title {
        req.title = Some(super::required_text("title", title)?);
    }
    super::position(req.position)?;

    db::update_chapter(&state.db, &id, &req)
        .await?
        .map(Json)
        .ok_or(AppError::NotFound("chapter"))
}

/// Deletes the chapter with all its notes and image files.
pub async fn delete(
    _: RequireAdmin,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<StatusCode> {
    let mut tx = state.db.begin().await?;
    let note_ids = db::chapter_note_ids(&mut *tx, &id).await?;
    if db::delete_chapter(&mut *tx, &id).await? == 0 {
        return Err(AppError::NotFound("chapter"));
    }
    tx.commit().await?;

    state.storage.delete_notes_logged(&note_ids).await;
    tracing::info!(chapter = %id, notes = note_ids.len(), "deleted chapter");
    Ok(StatusCode::NO_CONTENT)
}
