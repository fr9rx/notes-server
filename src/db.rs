//! All SQL lives here. Functions take any SQLite executor so they work on the
//! pool or inside a transaction (`&mut *tx`).

use std::str::FromStr;
use std::time::Duration;

use sqlx::SqliteExecutor;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePool, SqlitePoolOptions, SqliteSynchronous};

use crate::models::{
    Chapter, ChapterSummaryRow, Course, CourseSummaryRow, ImageRow, Note, UpdateChapter, UpdateCourse,
    UpdateNote,
};

pub async fn connect(url: &str) -> Result<SqlitePool, sqlx::Error> {
    let options = SqliteConnectOptions::from_str(url)?
        .create_if_missing(true)
        .foreign_keys(true)
        // WAL + NORMAL: durable across app crashes, far fewer fsyncs (SD-card friendly).
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Normal)
        .busy_timeout(Duration::from_secs(5));

    let path = options.get_filename();
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir)?;
    }

    SqlitePoolOptions::new()
        .max_connections(8)
        .connect_with(options)
        .await
}

pub async fn migrate(pool: &SqlitePool) -> Result<(), sqlx::migrate::MigrateError> {
    sqlx::migrate!("./migrations").run(pool).await
}

// ---- Courses ---------------------------------------------------------------

pub async fn list_courses(e: impl SqliteExecutor<'_>) -> sqlx::Result<Vec<CourseSummaryRow>> {
    sqlx::query_as(
        "SELECT c.*,
            (SELECT COUNT(*) FROM chapters ch WHERE ch.course_id = c.id) AS chapter_count,
            (SELECT COUNT(*) FROM notes n JOIN chapters ch ON ch.id = n.chapter_id
              WHERE ch.course_id = c.id) AS note_count,
            (SELECT COUNT(*) FROM images i JOIN notes n ON n.id = i.note_id
              JOIN chapters ch ON ch.id = n.chapter_id WHERE ch.course_id = c.id) AS image_count,
            (SELECT i.thumb_key FROM images i JOIN notes n ON n.id = i.note_id
              JOIN chapters ch ON ch.id = n.chapter_id WHERE ch.course_id = c.id
              ORDER BY n.created_at DESC, n.id DESC, i.position LIMIT 1) AS cover_thumb_key
         FROM courses c ORDER BY c.name COLLATE NOCASE, c.id",
    )
    .fetch_all(e)
    .await
}

pub async fn course_by_slug(e: impl SqliteExecutor<'_>, slug: &str) -> sqlx::Result<Option<Course>> {
    sqlx::query_as("SELECT * FROM courses WHERE slug = ?1")
        .bind(slug)
        .fetch_optional(e)
        .await
}

pub async fn insert_course(
    e: impl SqliteExecutor<'_>,
    id: &str,
    slug: &str,
    name: &str,
    description: &str,
) -> sqlx::Result<Course> {
    sqlx::query_as(
        "INSERT INTO courses (id, slug, name, description) VALUES (?1, ?2, ?3, ?4) RETURNING *",
    )
    .bind(id)
    .bind(slug)
    .bind(name)
    .bind(description)
    .fetch_one(e)
    .await
}

pub async fn update_course(
    e: impl SqliteExecutor<'_>,
    slug: &str,
    patch: &UpdateCourse,
) -> sqlx::Result<Option<Course>> {
    sqlx::query_as(
        "UPDATE courses SET
            slug = COALESCE(?1, slug),
            name = COALESCE(?2, name),
            description = COALESCE(?3, description),
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE slug = ?4 RETURNING *",
    )
    .bind(&patch.slug)
    .bind(&patch.name)
    .bind(&patch.description)
    .bind(slug)
    .fetch_optional(e)
    .await
}

pub async fn course_note_ids(e: impl SqliteExecutor<'_>, course_id: &str) -> sqlx::Result<Vec<String>> {
    sqlx::query_scalar(
        "SELECT n.id FROM notes n JOIN chapters c ON c.id = n.chapter_id WHERE c.course_id = ?1",
    )
    .bind(course_id)
    .fetch_all(e)
    .await
}

pub async fn delete_course(e: impl SqliteExecutor<'_>, id: &str) -> sqlx::Result<u64> {
    Ok(sqlx::query("DELETE FROM courses WHERE id = ?1")
        .bind(id)
        .execute(e)
        .await?
        .rows_affected())
}

// ---- Chapters --------------------------------------------------------------

pub async fn chapters_for_course(
    e: impl SqliteExecutor<'_>,
    course_id: &str,
) -> sqlx::Result<Vec<ChapterSummaryRow>> {
    sqlx::query_as(
        "SELECT ch.*,
            (SELECT COUNT(*) FROM notes n WHERE n.chapter_id = ch.id) AS note_count,
            (SELECT COUNT(*) FROM images i JOIN notes n ON n.id = i.note_id
              WHERE n.chapter_id = ch.id) AS image_count,
            (SELECT i.thumb_key FROM images i JOIN notes n ON n.id = i.note_id
              WHERE n.chapter_id = ch.id
              ORDER BY n.created_at DESC, n.id DESC, i.position LIMIT 1) AS cover_thumb_key
         FROM chapters ch WHERE ch.course_id = ?1 ORDER BY ch.position, ch.created_at, ch.id",
    )
    .bind(course_id)
    .fetch_all(e)
    .await
}

/// `(courses, chapters, notes, images)` totals.
pub async fn totals(e: impl SqliteExecutor<'_>) -> sqlx::Result<(i64, i64, i64, i64)> {
    sqlx::query_as(
        "SELECT (SELECT COUNT(*) FROM courses), (SELECT COUNT(*) FROM chapters),
                (SELECT COUNT(*) FROM notes), (SELECT COUNT(*) FROM images)",
    )
    .fetch_one(e)
    .await
}

pub async fn chapter_by_id(e: impl SqliteExecutor<'_>, id: &str) -> sqlx::Result<Option<Chapter>> {
    sqlx::query_as("SELECT * FROM chapters WHERE id = ?1")
        .bind(id)
        .fetch_optional(e)
        .await
}

/// `position = None` appends after the course's last chapter.
pub async fn insert_chapter(
    e: impl SqliteExecutor<'_>,
    id: &str,
    course_id: &str,
    title: &str,
    position: Option<i64>,
) -> sqlx::Result<Chapter> {
    sqlx::query_as(
        "INSERT INTO chapters (id, course_id, title, position)
         VALUES (?1, ?2, ?3, COALESCE(?4,
            (SELECT COALESCE(MAX(position), -1) + 1 FROM chapters WHERE course_id = ?2)))
         RETURNING *",
    )
    .bind(id)
    .bind(course_id)
    .bind(title)
    .bind(position)
    .fetch_one(e)
    .await
}

pub async fn update_chapter(
    e: impl SqliteExecutor<'_>,
    id: &str,
    patch: &UpdateChapter,
) -> sqlx::Result<Option<Chapter>> {
    sqlx::query_as(
        "UPDATE chapters SET
            title = COALESCE(?1, title),
            position = COALESCE(?2, position),
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = ?3 RETURNING *",
    )
    .bind(&patch.title)
    .bind(patch.position)
    .bind(id)
    .fetch_optional(e)
    .await
}

pub async fn chapter_note_ids(e: impl SqliteExecutor<'_>, chapter_id: &str) -> sqlx::Result<Vec<String>> {
    sqlx::query_scalar("SELECT id FROM notes WHERE chapter_id = ?1")
        .bind(chapter_id)
        .fetch_all(e)
        .await
}

pub async fn delete_chapter(e: impl SqliteExecutor<'_>, id: &str) -> sqlx::Result<u64> {
    Ok(sqlx::query("DELETE FROM chapters WHERE id = ?1")
        .bind(id)
        .execute(e)
        .await?
        .rows_affected())
}

// ---- Notes -----------------------------------------------------------------

pub async fn note_by_id(e: impl SqliteExecutor<'_>, id: &str) -> sqlx::Result<Option<Note>> {
    sqlx::query_as("SELECT * FROM notes WHERE id = ?1")
        .bind(id)
        .fetch_optional(e)
        .await
}

pub async fn insert_note(
    e: impl SqliteExecutor<'_>,
    id: &str,
    chapter_id: &str,
    title: &str,
    body: &str,
    author_name: Option<&str>,
) -> sqlx::Result<Note> {
    sqlx::query_as(
        "INSERT INTO notes (id, chapter_id, title, body, author_name)
         VALUES (?1, ?2, ?3, ?4, ?5) RETURNING *",
    )
    .bind(id)
    .bind(chapter_id)
    .bind(title)
    .bind(body)
    .bind(author_name)
    .fetch_one(e)
    .await
}

pub async fn update_note(
    e: impl SqliteExecutor<'_>,
    id: &str,
    patch: &UpdateNote,
) -> sqlx::Result<Option<Note>> {
    sqlx::query_as(
        "UPDATE notes SET
            title = COALESCE(?1, title),
            body = COALESCE(?2, body),
            author_name = CASE WHEN ?3 IS NULL THEN author_name ELSE NULLIF(?3, '') END,
            chapter_id = COALESCE(?4, chapter_id),
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = ?5 RETURNING *",
    )
    .bind(&patch.title)
    .bind(&patch.body)
    .bind(&patch.author_name)
    .bind(&patch.chapter_id)
    .bind(id)
    .fetch_optional(e)
    .await
}

pub async fn touch_note(e: impl SqliteExecutor<'_>, id: &str) -> sqlx::Result<()> {
    sqlx::query("UPDATE notes SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?1")
        .bind(id)
        .execute(e)
        .await?;
    Ok(())
}

pub async fn delete_note(e: impl SqliteExecutor<'_>, id: &str) -> sqlx::Result<u64> {
    Ok(sqlx::query("DELETE FROM notes WHERE id = ?1")
        .bind(id)
        .execute(e)
        .await?
        .rows_affected())
}

pub async fn count_notes(e: impl SqliteExecutor<'_>, chapter_id: &str) -> sqlx::Result<i64> {
    sqlx::query_scalar("SELECT COUNT(*) FROM notes WHERE chapter_id = ?1")
        .bind(chapter_id)
        .fetch_one(e)
        .await
}

/// Notes in upload order, or newest first with `newest_first` (UUIDv7 ids
/// break ties chronologically too).
pub async fn notes_page(
    e: impl SqliteExecutor<'_>,
    chapter_id: &str,
    limit: i64,
    offset: i64,
    newest_first: bool,
) -> sqlx::Result<Vec<Note>> {
    let sql = if newest_first {
        "SELECT * FROM notes WHERE chapter_id = ?1 ORDER BY created_at DESC, id DESC LIMIT ?2 OFFSET ?3"
    } else {
        "SELECT * FROM notes WHERE chapter_id = ?1 ORDER BY created_at, id LIMIT ?2 OFFSET ?3"
    };
    sqlx::query_as(sql)
        .bind(chapter_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(e)
        .await
}

/// Images for exactly the notes returned by [`notes_page`] with the same arguments.
pub async fn images_for_notes_page(
    e: impl SqliteExecutor<'_>,
    chapter_id: &str,
    limit: i64,
    offset: i64,
    newest_first: bool,
) -> sqlx::Result<Vec<ImageRow>> {
    let sql = if newest_first {
        "SELECT i.* FROM images i
         JOIN (SELECT id FROM notes WHERE chapter_id = ?1
               ORDER BY created_at DESC, id DESC LIMIT ?2 OFFSET ?3) n ON n.id = i.note_id
         ORDER BY i.note_id, i.position"
    } else {
        "SELECT i.* FROM images i
         JOIN (SELECT id FROM notes WHERE chapter_id = ?1
               ORDER BY created_at, id LIMIT ?2 OFFSET ?3) n ON n.id = i.note_id
         ORDER BY i.note_id, i.position"
    };
    sqlx::query_as(sql)
    .bind(chapter_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(e)
    .await
}

// ---- Images ----------------------------------------------------------------

pub async fn images_for_note(e: impl SqliteExecutor<'_>, note_id: &str) -> sqlx::Result<Vec<ImageRow>> {
    sqlx::query_as("SELECT * FROM images WHERE note_id = ?1 ORDER BY position, id")
        .bind(note_id)
        .fetch_all(e)
        .await
}

pub async fn next_image_position(e: impl SqliteExecutor<'_>, note_id: &str) -> sqlx::Result<i64> {
    sqlx::query_scalar("SELECT COALESCE(MAX(position), -1) + 1 FROM images WHERE note_id = ?1")
        .bind(note_id)
        .fetch_one(e)
        .await
}

pub async fn insert_image(e: impl SqliteExecutor<'_>, row: &ImageRow) -> sqlx::Result<ImageRow> {
    sqlx::query_as(
        "INSERT INTO images (id, note_id, position, original_filename, storage_key,
                             width, height, size_bytes, thumb_key, thumb_width, thumb_height)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11) RETURNING *",
    )
    .bind(&row.id)
    .bind(&row.note_id)
    .bind(row.position)
    .bind(&row.original_filename)
    .bind(&row.storage_key)
    .bind(row.width)
    .bind(row.height)
    .bind(row.size_bytes)
    .bind(&row.thumb_key)
    .bind(row.thumb_width)
    .bind(row.thumb_height)
    .fetch_one(e)
    .await
}

/// Returns the deleted image's `(storage_key, thumb_key)`.
pub async fn delete_image(
    e: impl SqliteExecutor<'_>,
    note_id: &str,
    image_id: &str,
) -> sqlx::Result<Option<(String, String)>> {
    sqlx::query_as(
        "DELETE FROM images WHERE id = ?1 AND note_id = ?2 RETURNING storage_key, thumb_key",
    )
    .bind(image_id)
    .bind(note_id)
    .fetch_optional(e)
    .await
}
