CREATE TABLE courses (
    id          TEXT PRIMARY KEY,
    slug        TEXT NOT NULL UNIQUE,
    name        TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE chapters (
    id         TEXT PRIMARY KEY,
    course_id  TEXT NOT NULL REFERENCES courses(id) ON DELETE CASCADE,
    title      TEXT NOT NULL,
    position   INTEGER NOT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
CREATE INDEX chapters_course ON chapters(course_id, position);

CREATE TABLE notes (
    id          TEXT PRIMARY KEY,
    chapter_id  TEXT NOT NULL REFERENCES chapters(id) ON DELETE CASCADE,
    title       TEXT NOT NULL,
    body        TEXT NOT NULL DEFAULT '',
    -- Free-text name typed in by the uploader; there are no user accounts.
    author_name TEXT,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
CREATE INDEX notes_chapter ON notes(chapter_id, created_at);

-- storage_key / thumb_key are paths relative to UPLOAD_DIR; URLs are built at
-- response time from PUBLIC_BASE_URL so the domain can change freely.
CREATE TABLE images (
    id                TEXT PRIMARY KEY,
    note_id           TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
    position          INTEGER NOT NULL,
    original_filename TEXT,
    storage_key       TEXT NOT NULL,
    width             INTEGER NOT NULL,
    height            INTEGER NOT NULL,
    size_bytes        INTEGER NOT NULL,
    thumb_key         TEXT NOT NULL,
    thumb_width       INTEGER NOT NULL,
    thumb_height      INTEGER NOT NULL,
    created_at        TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
CREATE INDEX images_note ON images(note_id, position);
