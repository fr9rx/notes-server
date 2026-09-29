-- A cover photo chosen by the admin (keys relative to UPLOAD_DIR). NULL means
-- "automatic": the newest photo in the course is used instead.
ALTER TABLE courses ADD COLUMN cover_key TEXT;
ALTER TABLE courses ADD COLUMN cover_thumb_key TEXT;
