//! Minimal text editing for forms: a cursor-aware line/multiline input and
//! a form made of labelled fields.

use std::path::PathBuf;

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldKind {
    Line,
    /// Rendered as bullets.
    Secret,
    /// Enter inserts a newline; submit with Ctrl+S.
    Multiline,
}

#[derive(Debug, Clone)]
pub struct Field {
    pub label: &'static str,
    pub hint: &'static str,
    pub kind: FieldKind,
    pub value: String,
    /// Cursor position in chars.
    pub cursor: usize,
}

impl Field {
    pub fn new(label: &'static str, kind: FieldKind, value: impl Into<String>) -> Self {
        let value = value.into();
        let cursor = value.chars().count();
        Self { label, hint: "", kind, value, cursor }
    }

    pub fn hint(mut self, hint: &'static str) -> Self {
        self.hint = hint;
        self
    }

    fn byte_index(&self, char_index: usize) -> usize {
        self.value
            .char_indices()
            .nth(char_index)
            .map_or(self.value.len(), |(i, _)| i)
    }

    pub fn insert(&mut self, text: &str) {
        let text: String = match self.kind {
            FieldKind::Multiline => text.replace("\r\n", "\n").replace('\r', "\n"),
            _ => text.replace(['\r', '\n'], ""),
        };
        let at = self.byte_index(self.cursor);
        self.value.insert_str(at, &text);
        self.cursor += text.chars().count();
    }

    pub fn backspace(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
            let at = self.byte_index(self.cursor);
            self.value.remove(at);
        }
    }

    pub fn delete(&mut self) {
        if self.cursor < self.value.chars().count() {
            let at = self.byte_index(self.cursor);
            self.value.remove(at);
        }
    }

    /// Text shown on screen (secrets masked).
    pub fn display(&self) -> String {
        match self.kind {
            FieldKind::Secret => "•".repeat(self.value.chars().count()),
            _ => self.value.clone(),
        }
    }

    /// (line, column) of the cursor, for placing the terminal cursor.
    pub fn cursor_line_col(&self) -> (usize, usize) {
        let before: String = self.value.chars().take(self.cursor).collect();
        let line = before.matches('\n').count();
        let col = before.rsplit('\n').next().map_or(0, |l| l.chars().count());
        (line, col)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormEvent {
    None,
    Submit,
    Cancel,
}

#[derive(Debug, Clone)]
pub struct Form {
    pub fields: Vec<Field>,
    pub active: usize,
}

impl Form {
    pub fn new(fields: Vec<Field>) -> Self {
        Self { fields, active: 0 }
    }

    pub fn value(&self, index: usize) -> &str {
        &self.fields[index].value
    }

    pub fn active_field(&mut self) -> &mut Field {
        &mut self.fields[self.active]
    }

    pub fn paste(&mut self, text: &str) {
        self.active_field().insert(text);
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> FormEvent {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let last = self.fields.len() - 1;
        let multiline = self.fields[self.active].kind == FieldKind::Multiline;
        match key.code {
            KeyCode::Esc => return FormEvent::Cancel,
            KeyCode::Char('s') if ctrl => return FormEvent::Submit,
            KeyCode::Char('u') if ctrl => {
                let f = self.active_field();
                f.value.clear();
                f.cursor = 0;
            }
            KeyCode::Enter if multiline && !ctrl => self.active_field().insert("\n"),
            KeyCode::Enter if self.active == last || ctrl => return FormEvent::Submit,
            KeyCode::Enter | KeyCode::Tab | KeyCode::Down => self.active = (self.active + 1).min(last),
            KeyCode::BackTab | KeyCode::Up => self.active = self.active.saturating_sub(1),
            KeyCode::Left => {
                let f = self.active_field();
                f.cursor = f.cursor.saturating_sub(1);
            }
            KeyCode::Right => {
                let f = self.active_field();
                f.cursor = (f.cursor + 1).min(f.value.chars().count());
            }
            KeyCode::Home => self.active_field().cursor = 0,
            KeyCode::End => {
                let f = self.active_field();
                f.cursor = f.value.chars().count();
            }
            KeyCode::Backspace => self.active_field().backspace(),
            KeyCode::Delete => self.active_field().delete(),
            KeyCode::Char(c) if !ctrl => self.active_field().insert(c.encode_utf8(&mut [0; 4])),
            _ => {}
        }
        FormEvent::None
    }
}

const IMAGE_EXTENSIONS: [&str; 5] = ["jpg", "jpeg", "png", "webp", "gif"];

/// Parses a list of image paths separated by `;` or newlines. Surrounding
/// quotes (added by drag-and-drop into Windows Terminal) are stripped and a
/// directory expands to the images directly inside it, sorted by name.
pub fn parse_image_paths(input: &str) -> Result<Vec<PathBuf>, String> {
    let mut out = Vec::new();
    for raw in input.split([';', '\n']) {
        let raw = raw.trim().trim_matches(|c| c == '"' || c == '\'').trim();
        if raw.is_empty() {
            continue;
        }
        let path = PathBuf::from(raw);
        if path.is_dir() {
            let mut images: Vec<PathBuf> = std::fs::read_dir(&path)
                .map_err(|e| format!("reading folder {}: {e}", path.display()))?
                .flatten()
                .map(|e| e.path())
                .filter(|p| {
                    p.is_file()
                        && p.extension()
                            .and_then(|e| e.to_str())
                            .is_some_and(|e| IMAGE_EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
                })
                .collect();
            if images.is_empty() {
                return Err(format!("no images in folder {}", path.display()));
            }
            images.sort();
            out.extend(images);
        } else if path.is_file() {
            out.push(path);
        } else {
            return Err(format!("file not found: {}", path.display()));
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::crossterm::event::KeyEvent;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn editing_with_cursor_and_unicode() {
        let mut f = Field::new("t", FieldKind::Line, "héllo");
        f.cursor = 1;
        f.insert("ö");
        assert_eq!(f.value, "höéllo");
        f.backspace();
        f.backspace();
        assert_eq!(f.value, "éllo");
        assert_eq!(f.cursor, 0);
        f.delete();
        assert_eq!(f.value, "llo");
        f.insert("a\nb"); // newlines dropped in single-line fields
        assert_eq!(f.value, "abllo");
    }

    #[test]
    fn multiline_cursor_position() {
        let mut f = Field::new("b", FieldKind::Multiline, "");
        f.insert("ab\r\ncd");
        assert_eq!(f.value, "ab\ncd");
        assert_eq!(f.cursor_line_col(), (1, 2));
        f.cursor = 1;
        assert_eq!(f.cursor_line_col(), (0, 1));
    }

    #[test]
    fn form_navigation_and_submit() {
        let mut form = Form::new(vec![
            Field::new("a", FieldKind::Line, ""),
            Field::new("b", FieldKind::Multiline, ""),
            Field::new("c", FieldKind::Line, ""),
        ]);
        form.handle_key(key(KeyCode::Char('x')));
        assert_eq!(form.handle_key(key(KeyCode::Enter)), FormEvent::None); // next field
        assert_eq!(form.active, 1);
        form.handle_key(key(KeyCode::Char('1')));
        form.handle_key(key(KeyCode::Enter)); // newline in multiline
        form.handle_key(key(KeyCode::Char('2')));
        assert_eq!(form.value(1), "1\n2");
        form.handle_key(key(KeyCode::Tab));
        assert_eq!(form.handle_key(key(KeyCode::Enter)), FormEvent::Submit); // last field
        assert_eq!(
            form.handle_key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL)),
            FormEvent::Submit
        );
        assert_eq!(form.handle_key(key(KeyCode::Esc)), FormEvent::Cancel);
        assert_eq!(form.value(0), "x");
    }

    #[test]
    fn secret_is_masked() {
        let f = Field::new("t", FieldKind::Secret, "abc");
        assert_eq!(f.display(), "•••");
    }

    #[test]
    fn image_path_parsing() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("b.JPG");
        let b = dir.path().join("a.png");
        std::fs::write(&a, b"x").unwrap();
        std::fs::write(&b, b"x").unwrap();
        std::fs::write(dir.path().join("notes.txt"), b"x").unwrap();

        // Folder expands to its images, sorted; quotes stripped.
        let input = format!("\"{}\"", dir.path().display());
        assert_eq!(parse_image_paths(&input).unwrap(), vec![b.clone(), a.clone()]);
        let input = format!("{} ; '{}'", a.display(), b.display());
        assert_eq!(parse_image_paths(&input).unwrap(), vec![a, b]);
        assert!(parse_image_paths("C:/definitely/missing.jpg").is_err());
        assert!(parse_image_paths(" ; ").unwrap().is_empty());
    }
}
