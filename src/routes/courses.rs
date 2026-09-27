use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use uuid::Uuid;

use crate::AppState;
use crate::auth::RequireAdmin;
use crate::db;
use crate::error::{AppError, AppResult, is_unique_violation};
use crate::models::{Course, CourseDetail, CreateCourse, UpdateCourse};

pub async fn list(State(state): State<AppState>) -> AppResult<Json<Vec<Course>>> {
    Ok(Json(db::list_courses(&state.db).await?))
}

pub async fn get(
    State(state): State<AppState>,
    Path(slug): Path<String>,
) -> AppResult<Json<CourseDetail>> {
    let course = db::course_by_slug(&state.db, &slug)
        .await?
        .ok_or(AppError::NotFound("course"))?;
    let chapters = db::chapters_for_course(&state.db, &course.id).await?;
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
    tracing::info!(course = %slug, notes = note_ids.len(), "deleted course");
    Ok(StatusCode::NO_CONTENT)
}

fn slug_taken(slug: &str) -> AppError {
    AppError::Conflict(format!("a course with slug `{slug}` already exists"))
}
