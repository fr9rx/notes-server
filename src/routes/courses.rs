use axum::Json;
use axum::extract::{Multipart, Path, State};
use axum::http::StatusCode;
use uuid::Uuid;

use crate::AppState;
use crate::auth::RequireAdmin;
use crate::db;
use crate::error::{AppError, AppResult, is_unique_violation};
use crate::models::{ChapterSummary, Course, CourseDetail, CourseSummary, CreateCourse, UpdateCourse};
use crate::storage::{cover_key, cover_thumb_key};

pub async fn list(State(state): State<AppState>) -> AppResult<Json<Vec<CourseSummary>>> {
    let rows = db::list_courses(&state.db).await?;
    Ok(Json(
        rows.into_iter()
            .map(|r| CourseSummary::new(r, &state.storage))
            .collect(),
    ))
}

pub async fn get(
    State(state): State<AppState>,
    Path(slug): Path<String>,
) -> AppResult<Json<CourseDetail>> {
    let course = summary(&state, &slug).await?;
    let chapters = db::chapters_for_course(&state.db, &course.course.id)
        .await?
        .into_iter()
        .map(|r| ChapterSummary::new(r, &state.storage))
        .collect();
    Ok(Json(CourseDetail { course, chapters }))
}

pub async fn create(
    _: RequireAdmin,
    State(state): State<AppState>,
    Json(req): Json<CreateCourse>,
) -> AppResult<(StatusCode, Json<Course>)> {
    super::slug(&req.slug)?;
    let name = super::required_text("name", &req.name)?;
    super::description(&req.description)?;

    let id = Uuid::now_v7().to_string();
    match db::insert_course(&state.db, &id, &req.slug, &name, &req.description).await {
        Ok(course) => Ok((StatusCode::CREATED, Json(course))),
        Err(e) if is_unique_violation(&e) => Err(slug_taken(&req.slug)),
        Err(e) => Err(e.into()),
    }
}

pub async fn update(
    _: RequireAdmin,
    State(state): State<AppState>,
    Path(slug): Path<String>,
    Json(mut req): Json<UpdateCourse>,
) -> AppResult<Json<Course>> {
    if let Some(new_slug) = &req.slug {
        super::slug(new_slug)?;
    }
    if let Some(name) = &req.name {
        req.name = Some(super::required_text("name", name)?);
    }
    if let Some(desc) = &req.description {
        super::description(desc)?;
    }

    match db::update_course(&state.db, &slug, &req).await {
        Ok(Some(course)) => Ok(Json(course)),
        Ok(None) => Err(AppError::NotFound("course")),
        Err(e) if is_unique_violation(&e) => Err(slug_taken(req.slug.as_deref().unwrap_or(&slug))),
        Err(e) => Err(e.into()),
    }
}

/// Deletes the course with all its chapters, notes and image files.
pub async fn delete(
    _: RequireAdmin,
    State(state): State<AppState>,
    Path(slug): Path<String>,
) -> AppResult<StatusCode> {
    let mut tx = state.db.begin().await?;
    let course = db::course_by_slug(&mut *tx, &slug)
        .await?
        .ok_or(AppError::NotFound("course"))?;
    let note_ids = db::course_note_ids(&mut *tx, &course.id).await?;
    db::delete_course(&mut *tx, &course.id).await?;
    tx.commit().await?;

    // Rows first, then files: the worst case is an orphaned file, never a
    // row pointing at nothing.
    state.storage.delete_notes_logged(&note_ids).await;
    state.storage.delete_course_covers_logged(&course.id).await;
    tracing::info!(course = %slug, notes = note_ids.len(), "deleted course");
    Ok(StatusCode::NO_CONTENT)
}

/// Sets the course's cover photo from a multipart upload with one `image`,
/// replacing any previous cover. It is resized like note photos.
pub async fn set_cover(
    _: RequireAdmin,
    State(state): State<AppState>,
    Path(slug): Path<String>,
    multipart: Multipart,
) -> AppResult<Json<CourseSummary>> {
    let course = db::course_by_slug(&state.db, &slug)
        .await?
        .ok_or(AppError::NotFound("course"))?;
    let p = super::notes::process_single_image(&state, multipart).await?;

    let cover_id = Uuid::now_v7().to_string();
    let key = cover_key(&course.id, &cover_id);
    let thumb = cover_thumb_key(&course.id, &cover_id);
    let mut written = Vec::with_capacity(2);
    for (k, bytes) in [(&key, &p.main.bytes), (&thumb, &p.thumb.bytes)] {
        if let Err(e) = state.storage.put(k, bytes).await {
            state.storage.delete_keys_logged(&written).await;
            return Err(e.into());
        }
        written.push(k.clone());
    }

    match db::replace_course_cover(&state.db, &course.id, Some((&key, &thumb))).await {
        Ok(Some(old)) => state.storage.delete_keys_logged(&old).await,
        result => {
            state.storage.delete_keys_logged(&written).await;
            result?; // a DB error; otherwise the course was deleted meanwhile
            return Err(AppError::NotFound("course"));
        }
    }
    tracing::info!(course = %slug, width = p.main.width, height = p.main.height, "set course cover");
    Ok(Json(summary(&state, &slug).await?))
}

/// Removes the admin's cover photo; the course goes back to showing its
/// newest photo.
pub async fn delete_cover(
    _: RequireAdmin,
    State(state): State<AppState>,
    Path(slug): Path<String>,
) -> AppResult<StatusCode> {
    let course = db::course_by_slug(&state.db, &slug)
        .await?
        .ok_or(AppError::NotFound("course"))?;
    let old = db::replace_course_cover(&state.db, &course.id, None)
        .await?
        .ok_or(AppError::NotFound("course"))?;
    state.storage.delete_keys_logged(&old).await;
    Ok(StatusCode::NO_CONTENT)
}

async fn summary(state: &AppState, slug: &str) -> AppResult<CourseSummary> {
    let row = db::course_summary_by_slug(&state.db, slug)
        .await?
        .ok_or(AppError::NotFound("course"))?;
    Ok(CourseSummary::new(row, &state.storage))
}

fn slug_taken(slug: &str) -> AppError {
    AppError::Conflict(format!("a course with slug `{slug}` already exists"))
}
