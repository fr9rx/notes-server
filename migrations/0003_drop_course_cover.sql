-- The admin-chosen course cover (0002) was removed again; covers are always
-- the course's newest photo. 0002 stays because databases have applied it.
ALTER TABLE courses DROP COLUMN cover_key;
ALTER TABLE courses DROP COLUMN cover_thumb_key;
