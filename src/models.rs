use serde::{Deserialize, Serialize};
use sqlx::FromRow;

use crate::storage::LocalStorage;

// ---- Rows ------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Course {
    pub id: String,
    pub slug: String,
    pub name: String,
    pub description: String,
    pub created_at: String,
    pub updated_at: String,
    /// Admin-chosen cover photo (storage keys); exposed as URLs on [`CourseSummary`].
    #[serde(skip)]
    pub cover_key: Option<String>,
    #[serde(skip)]
    pub cover_thumb_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Chapter {
    pub id: String,
    pub course_id: String,
    pub title: String,
    pub position: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Note {
    pub id: String,
    pub chapter_id: String,
    pub title: String,
    pub body: String,
    pub author_name: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, FromRow)]
pub struct ImageRow {
    pub id: String,
    pub note_id: String,
    pub position: i64,
    pub original_filename: Option<String>,
    pub storage_key: String,
    pub width: i64,
    pub height: i64,
    pub size_bytes: i64,
    pub thumb_key: String,
    pub thumb_width: i64,
    pub thumb_height: i64,
    pub created_at: String,
}

// ---- Responses -------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageDto {
    pub id: String,
    pub position: i64,
    pub url: String,
    pub thumb_url: String,
    pub width: i64,
    pub height: i64,
    pub thumb_width: i64,
    pub thumb_height: i64,
    pub size_bytes: i64,
    pub original_filename: Option<String>,
    pub created_at: String,
}

impl ImageDto {
    pub fn new(row: ImageRow, storage: &LocalStorage) -> Self {
        Self {
            url: storage.url_for(&row.storage_key),
            thumb_url: storage.url_for(&row.thumb_key),
            id: row.id,
            position: row.position,
            width: row.width,
            height: row.height,
            thumb_width: row.thumb_width,
            thumb_height: row.thumb_height,
            size_bytes: row.size_bytes,
            original_filename: row.original_filename,
            created_at: row.created_at,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoteDto {
    #[serde(flatten)]
    pub note: Note,
    pub images: Vec<ImageDto>,
}

impl NoteDto {
    pub fn new(note: Note, images: Vec<ImageRow>, storage: &LocalStorage) -> Self {
        Self {
            note,
            images: images
                .into_iter()
                .map(|row| ImageDto::new(row, storage))
                .collect(),
        }
    }
}

/// Course listing row: the course plus counts and its newest image, the
/// automatic cover used when the admin hasn't set one.
#[derive(Debug, Clone, FromRow)]
pub struct CourseSummaryRow {
    #[sqlx(flatten)]
    pub course: Course,
    pub chapter_count: i64,
    pub note_count: i64,
    pub image_count: i64,
    pub auto_cover_key: Option<String>,
    pub auto_cover_thumb_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CourseSummary {
    #[serde(flatten)]
    pub course: Course,
    pub chapter_count: i64,
    pub note_count: i64,
    pub image_count: i64,
    /// The admin's cover photo if set, else the course's newest photo.
    pub cover_url: Option<String>,
    pub cover_thumb_url: Option<String>,
    /// Whether the cover was chosen by the admin (vs. picked automatically).
    pub custom_cover: bool,
}

impl CourseSummary {
    pub fn new(row: CourseSummaryRow, storage: &LocalStorage) -> Self {
        let c = &row.course;
        let custom_cover = c.cover_key.is_some() && c.cover_thumb_key.is_some();
        let (main, thumb) = if custom_cover {
            (c.cover_key.clone(), c.cover_thumb_key.clone())
        } else {
            (row.auto_cover_key, row.auto_cover_thumb_key)
        };
        Self {
            cover_url: main.map(|k| storage.url_for(&k)),
            cover_thumb_url: thumb.map(|k| storage.url_for(&k)),
            custom_cover,
            course: row.course,
            chapter_count: row.chapter_count,
            note_count: row.note_count,
            image_count: row.image_count,
        }
    }
}

/// Chapter listing row: the chapter plus counts and the newest image as a cover.
#[derive(Debug, Clone, FromRow)]
pub struct ChapterSummaryRow {
    #[sqlx(flatten)]
    pub chapter: Chapter,
    pub note_count: i64,
    pub image_count: i64,
    pub cover_thumb_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChapterSummary {
    #[serde(flatten)]
    pub chapter: Chapter,
    pub note_count: i64,
    pub image_count: i64,
    pub cover_thumb_url: Option<String>,
}

impl ChapterSummary {
    pub fn new(row: ChapterSummaryRow, storage: &LocalStorage) -> Self {
        Self {
            cover_thumb_url: row.cover_thumb_key.map(|k| storage.url_for(&k)),
            chapter: row.chapter,
            note_count: row.note_count,
            image_count: row.image_count,
        }
    }
}

/// `GET /api/stats`: totals plus the live numbers the LED matrix shows.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Stats {
    pub courses: i64,
    pub chapters: i64,
    pub notes: i64,
    pub images: i64,
    pub uptime_secs: u64,
    /// "starting" | "ok" | "warning" | "error"
    pub status: String,
    /// Requests per second over the last 13 seconds, oldest first.
    pub requests: Vec<u64>,
    /// Uploaded images per second over the last 13 seconds, oldest first.
    pub uploads: Vec<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CourseDetail {
    #[serde(flatten)]
    pub course: CourseSummary,
    pub chapters: Vec<ChapterSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChapterDetail {
    #[serde(flatten)]
    pub chapter: Chapter,
    pub notes: Vec<NoteDto>,
    pub total_notes: i64,
    pub limit: i64,
    pub offset: i64,
}

// ---- Requests --------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateCourse {
    pub slug: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpdateCourse {
    pub slug: Option<String>,
    pub name: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateChapter {
    pub title: String,
    pub position: Option<i64>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpdateChapter {
    pub title: Option<String>,
    pub position: Option<i64>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpdateNote {
    pub title: Option<String>,
    pub body: Option<String>,
    /// An empty string clears the author name.
    pub author_name: Option<String>,
    /// Moves the note to another chapter.
    pub chapter_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Page {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    /// "asc" (default, oldest first) or "desc" (newest first).
    pub order: Option<String>,
}
