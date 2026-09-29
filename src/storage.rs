//! Local-disk blob storage. Keys are always server-generated
//! (`notes/{note_id}/{image_id}.jpg`), never taken from user input, so there
//! is no path-traversal surface. All file access goes through this module so
//! an object-store backend could replace it without touching the handlers.

use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct LocalStorage {
    root: Arc<PathBuf>,
    base_url: Arc<str>,
}

impl LocalStorage {
    /// `public_base_url` is the externally visible origin, e.g. `https://notes.example.com`.
    pub fn new(root: impl Into<PathBuf>, public_base_url: &str) -> Self {
        Self {
            root: Arc::new(root.into()),
            base_url: public_base_url.trim_end_matches('/').into(),
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn url_for(&self, key: &str) -> String {
        format!("{}/files/{key}", self.base_url)
    }

    /// Writes to a temporary sibling and renames, so a partially written file
    /// is never served.
    pub async fn put(&self, key: &str, bytes: &[u8]) -> io::Result<()> {
        let path = self.root.join(key);
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let mut tmp = path.clone().into_os_string();
        tmp.push(".tmp");
        tokio::fs::write(&tmp, bytes).await?;
        if let Err(e) = tokio::fs::rename(&tmp, &path).await {
            let _ = tokio::fs::remove_file(&tmp).await;
            return Err(e);
        }
        Ok(())
    }

    /// Deletes one object; a missing file is not an error.
    pub async fn delete(&self, key: &str) -> io::Result<()> {
        ignore_not_found(tokio::fs::remove_file(self.root.join(key)).await)
    }

    /// Deletes every object belonging to a note.
    pub async fn delete_note(&self, note_id: &str) -> io::Result<()> {
        ignore_not_found(tokio::fs::remove_dir_all(self.root.join(note_dir(note_id))).await)
    }

    /// Best-effort bulk delete used after the DB rows are already gone; failures
    /// only leave orphaned files, so they are logged rather than returned.
    pub async fn delete_notes_logged(&self, note_ids: &[String]) {
        for id in note_ids {
            if let Err(e) = self.delete_note(id).await {
                tracing::warn!(note_id = %id, error = %e, "failed to delete note files");
            }
        }
    }

    pub async fn delete_keys_logged(&self, keys: &[String]) {
        for key in keys {
            if let Err(e) = self.delete(key).await {
                tracing::warn!(%key, error = %e, "failed to delete file");
            }
        }
    }

    /// Free bytes on the volume holding the upload directory.
    pub fn available_bytes(&self) -> io::Result<u64> {
        fs4::available_space(self.root.as_path())
    }
}

fn ignore_not_found(r: io::Result<()>) -> io::Result<()> {
    match r {
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        other => other,
    }
}

fn note_dir(note_id: &str) -> String {
    format!("notes/{note_id}")
}

pub fn image_key(note_id: &str, image_id: &str) -> String {
    format!("{}/{image_id}.jpg", note_dir(note_id))
}

pub fn thumb_key(note_id: &str, image_id: &str) -> String {
    format!("{}/{image_id}_thumb.jpg", note_dir(note_id))
}
