//! State and behaviour of the admin TUI, independent of rendering.
//!
//! Network calls are synchronous from the UI's point of view: a handler
//! queues a task with [`App::defer`], the main loop draws one frame (showing
//! the "busy" status) and then runs it.

use std::collections::HashMap;
use std::path::PathBuf;

use notes_server::models::{
    Chapter, Course, CreateChapter, CreateCourse, ImageDto, NoteDto, UpdateChapter, UpdateCourse,
    UpdateNote,
};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::widgets::ListState;
use tokio::runtime::Runtime;

use crate::api::Api;
use crate::input::{Field, FieldKind, Form, FormEvent, parse_image_paths};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Courses,
    Chapters,
    Notes,
    Images,
}

impl Focus {
    const ORDER: [Focus; 4] = [Focus::Courses, Focus::Chapters, Focus::Notes, Focus::Images];

    fn step(self, delta: isize) -> Self {
        let i = Self::ORDER.iter().position(|f| *f == self).unwrap_or(0) as isize;
        Self::ORDER[(i + delta).clamp(0, Self::ORDER.len() as isize - 1) as usize]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusKind {
    Info,
    Error,
    Busy,
}

#[derive(Debug, Clone)]
pub enum FormAction {
    NewCourse,
    EditCourse { slug: String },
    NewChapter { course_slug: String },
    EditChapter { id: String },
    NewNote { chapter_id: String },
    EditNote { id: String },
    AddImages { note_id: String },
}

#[derive(Debug, Clone)]
pub enum DeleteTarget {
    Course { id: String, slug: String },
    Chapter { id: String },
    Note { id: String },
    Image { note_id: String, image_id: String },
}

#[derive(Debug, Clone)]
pub struct PickItem {
    pub label: String,
    pub chapter_id: String,
    pub current: bool,
}

pub enum Modal {
    Form { title: String, form: Form, action: FormAction },
    Confirm { message: String, action: DeleteTarget },
    MoveNote { note_id: String, from_chapter: String, items: Vec<PickItem>, state: ListState },
    Help,
}

type Task = Box<dyn FnOnce(&mut App)>;

pub struct App {
    rt: Runtime,
    pub api: Option<Api>,
    ca_cert: Option<PathBuf>,
    insecure: bool,
    pub login: Form,

    pub courses: Vec<Course>,
    pub course_state: ListState,
    pub chapters: Vec<Chapter>,
    pub chapter_state: ListState,
    pub notes: Vec<NoteDto>,
    pub note_state: ListState,
    pub image_state: ListState,
    /// course id -> chapters
    chapters_cache: HashMap<String, Vec<Chapter>>,
    /// chapter id -> notes
    notes_cache: HashMap<String, Vec<NoteDto>>,

    pub focus: Focus,
    pub modal: Option<Modal>,
    pub status: Option<(StatusKind, String)>,
    pending: Option<Task>,
    pub quit: bool,
}

impl App {
    pub fn new(rt: Runtime, url: String, token: String, ca_cert: Option<PathBuf>, insecure: bool) -> Self {
        let auto_connect = !url.is_empty() && !token.is_empty();
        let ca_text = ca_cert.as_ref().map(|p| p.display().to_string()).unwrap_or_default();
        let mut login = Form::new(vec![
            Field::new("Server URL", FieldKind::Line, url).hint("e.g. https://notes.example.com"),
            Field::new("Admin token", FieldKind::Secret, token).hint("ADMIN_TOKEN from the server's env file"),
            Field::new("CA certificate (optional)", FieldKind::Line, ca_text)
                .hint("cert.pem, for a self-signed server"),
        ]);
        login.active = if login.value(0).is_empty() { 0 } else { 1 };

        let mut app = Self {
            rt,
            api: None,
            ca_cert,
            insecure,
            login,
            courses: Vec::new(),
            course_state: ListState::default(),
            chapters: Vec::new(),
            chapter_state: ListState::default(),
            notes: Vec::new(),
            note_state: ListState::default(),
            image_state: ListState::default(),
            chapters_cache: HashMap::new(),
            notes_cache: HashMap::new(),
            focus: Focus::Courses,
            modal: None,
            status: None,
            pending: None,
            quit: false,
        };
        if auto_connect {
            app.defer("Connecting…", App::connect);
        }
        app
    }

    // ---- Task plumbing -------------------------------------------------------

    fn defer(&mut self, label: &str, task: impl FnOnce(&mut App) + 'static) {
        self.status = Some((StatusKind::Busy, label.to_owned()));
        self.pending = Some(Box::new(task));
    }

    /// Runs the queued task, if any. Returns whether one ran.
    pub fn run_pending(&mut self) -> bool {
        let Some(task) = self.pending.take() else { return false };
        task(self);
        if self.pending.is_none() && matches!(self.status, Some((StatusKind::Busy, _))) {
            self.status = None;
        }
        true
    }

    pub fn is_busy(&self) -> bool {
        self.pending.is_some()
    }

    fn info(&mut self, msg: impl Into<String>) {
        self.status = Some((StatusKind::Info, msg.into()));
    }

    fn fail(&mut self, msg: impl std::fmt::Display) {
        self.status = Some((StatusKind::Error, msg.to_string()));
    }

    fn api(&self) -> Api {
        self.api.clone().expect("connected")
    }

    // ---- Selection helpers ---------------------------------------------------

    pub fn selected_course(&self) -> Option<&Course> {
        self.course_state.selected().and_then(|i| self.courses.get(i))
    }

    pub fn selected_chapter(&self) -> Option<&Chapter> {
        self.chapter_state.selected().and_then(|i| self.chapters.get(i))
    }

    pub fn selected_note(&self) -> Option<&NoteDto> {
        self.note_state.selected().and_then(|i| self.notes.get(i))
    }

    pub fn selected_image(&self) -> Option<&ImageDto> {
        let note = self.selected_note()?;
        self.image_state.selected().and_then(|i| note.images.get(i))
    }

    /// Number of notes in a chapter, if known.
    pub fn cached_note_count(&self, chapter_id: &str) -> Option<usize> {
        self.notes_cache.get(chapter_id).map(Vec::len)
    }

    // ---- Loading -------------------------------------------------------------

    fn connect(&mut self) {
        let url = self.login.value(0).trim().to_owned();
        let token = self.login.value(1).trim().to_owned();
        let ca = self.login.value(2).trim();
        let ca = if ca.is_empty() { None } else { Some(PathBuf::from(ca)) };
        if token.is_empty() {
            self.login.active = 1;
            return self.fail("the admin token is required");
        }
        let api = match Api::new(&url, &token, ca.as_deref(), self.insecure) {
            Ok(api) => api,
            Err(e) => return self.fail(e),
        };
        if let Err(e) = self.rt.block_on(api.check_auth()) {
            return self.fail(format!("cannot connect: {e}"));
        }
        self.ca_cert = ca;
        self.info(format!("Connected to {}", api.base()));
        self.api = Some(api);
        self.load_courses(None);
    }

    pub fn reload_all(&mut self) {
        self.chapters_cache.clear();
        self.notes_cache.clear();
        let (course, chapter, note) = (
            self.selected_course().map(|c| c.id.clone()),
            self.selected_chapter().map(|c| c.id.clone()),
            self.selected_note().map(|n| n.note.id.clone()),
        );
        let api = self.api();
        match self.rt.block_on(api.courses()) {
            Ok(courses) => self.courses = courses,
            Err(e) => return self.fail(e),
        }
        select_by(&mut self.course_state, &self.courses, course.as_deref(), |c| &c.id);
        self.show_chapters(chapter);
        // show_chapters loaded the notes; restore the note selection by id.
        select_by(&mut self.note_state, &self.notes, note.as_deref(), |n| &n.note.id);
        self.reset_images();
        if !matches!(self.status, Some((StatusKind::Error, _))) {
            self.info("Refreshed");
        }
    }

    fn load_courses(&mut self, keep: Option<String>) {
        let api = self.api();
        match self.rt.block_on(api.courses()) {
            Ok(courses) => {
                self.courses = courses;
                select_by(&mut self.course_state, &self.courses, keep.as_deref(), |c| &c.id);
                self.show_chapters(None);
            }
            Err(e) => self.fail(e),
        }
    }

    fn show_chapters(&mut self, keep: Option<String>) {
        let Some(course) = self.selected_course().cloned() else {
            self.chapters.clear();
            self.chapter_state.select(None);
            return self.show_notes(None);
        };
        if !self.chapters_cache.contains_key(&course.id) {
            let api = self.api();
            match self.rt.block_on(api.course(&course.slug)) {
                Ok(detail) => {
                    let chapters = detail.chapters.into_iter().map(|c| c.chapter).collect();
                    self.chapters_cache.insert(course.id.clone(), chapters);
                }
                Err(e) => {
                    self.chapters.clear();
                    self.chapter_state.select(None);
                    self.show_notes(None);
                    return self.fail(e);
                }
            }
        }
        self.chapters = self.chapters_cache[&course.id].clone();
        select_by(&mut self.chapter_state, &self.chapters, keep.as_deref(), |c| &c.id);
        self.show_notes(None);
    }

    fn show_notes(&mut self, keep: Option<String>) {
        let Some(chapter) = self.selected_chapter().cloned() else {
            self.notes.clear();
            self.note_state.select(None);
            return self.reset_images();
        };
        if !self.notes_cache.contains_key(&chapter.id) {
            let api = self.api();
            match self.rt.block_on(api.chapter_notes(&chapter.id)) {
                Ok(notes) => {
                    self.notes_cache.insert(chapter.id.clone(), notes);
                }
                Err(e) => {
                    self.notes.clear();
                    self.note_state.select(None);
                    self.reset_images();
                    return self.fail(e);
                }
            }
        }
        self.notes = self.notes_cache[&chapter.id].clone();
        select_by(&mut self.note_state, &self.notes, keep.as_deref(), |n| &n.note.id);
        self.reset_images();
    }

    fn reset_images(&mut self) {
        let has = self.selected_note().is_some_and(|n| !n.images.is_empty());
        let keep = self.image_state.selected();
        let len = self.selected_note().map_or(0, |n| n.images.len());
        self.image_state
            .select(has.then(|| keep.unwrap_or(0).min(len.saturating_sub(1))));
    }

    // ---- Input ---------------------------------------------------------------

    pub fn on_paste(&mut self, text: &str) {
        if self.is_busy() {
            return;
        }
        if self.api.is_none() {
            self.login.paste(text);
        } else if let Some(Modal::Form { form, .. }) = &mut self.modal {
            form.paste(text);
        }
    }

    pub fn on_key(&mut self, key: KeyEvent) {
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.quit = true;
            return;
        }
        if self.is_busy() {
            return;
        }
        if matches!(self.status, Some((StatusKind::Info | StatusKind::Error, _))) {
            self.status = None;
        }

        if self.api.is_none() {
            match self.login.handle_key(key) {
                FormEvent::Submit => self.defer("Connecting…", App::connect),
                FormEvent::Cancel => self.quit = true,
                FormEvent::None => {}
            }
            return;
        }

        match self.modal.take() {
            None => self.on_browse_key(key),
            Some(Modal::Help) => {} // any key closes
            Some(Modal::Form { title, mut form, action }) => match form.handle_key(key) {
                FormEvent::Cancel => {}
                FormEvent::Submit => self.submit_form(title, form, action),
                FormEvent::None => self.modal = Some(Modal::Form { title, form, action }),
            },
            Some(Modal::Confirm { message, action }) => match key.code {
                KeyCode::Char('y' | 'Y') => self.defer("Deleting…", move |app| app.confirm(action)),
                KeyCode::Char('n' | 'N') | KeyCode::Esc => {}
                _ => self.modal = Some(Modal::Confirm { message, action }),
            },
            Some(Modal::MoveNote { note_id, from_chapter, items, mut state }) => match key.code {
                KeyCode::Esc => {}
                KeyCode::Enter => {
                    if let Some(item) = state.selected().and_then(|i| items.get(i)) {
                        let to = item.chapter_id.clone();
                        if to == from_chapter {
                            self.info("The note is already in that chapter");
                        } else {
                            self.defer("Moving note…", move |app| app.move_note(note_id, from_chapter, to));
                        }
                    }
                }
                code => {
                    move_state(&mut state, items.len(), nav_delta(code));
                    self.modal = Some(Modal::MoveNote { note_id, from_chapter, items, state });
                }
            },
        }
    }

    fn on_browse_key(&mut self, key: KeyEvent) {
        let shift = key.modifiers.contains(KeyModifiers::SHIFT);
        match key.code {
            KeyCode::Char('q') => self.quit = true,
            KeyCode::Char('?') => self.modal = Some(Modal::Help),
            KeyCode::Char('r') | KeyCode::F(5) => self.defer("Refreshing…", App::reload_all),
            KeyCode::Left | KeyCode::Char('h') | KeyCode::BackTab => self.focus = self.focus.step(-1),
            KeyCode::Right | KeyCode::Char('l') | KeyCode::Tab => self.focus = self.focus.step(1),
            KeyCode::Char('K') => self.reorder_chapter(-1),
            KeyCode::Char('J') => self.reorder_chapter(1),
            KeyCode::Up if shift => self.reorder_chapter(-1),
            KeyCode::Down if shift => self.reorder_chapter(1),
            KeyCode::Char('a' | 'n') => self.open_add(),
            KeyCode::Char('i') => self.open_add_images(),
            KeyCode::Char('e') => self.open_edit(),
            KeyCode::Enter if self.focus == Focus::Images => self.open_image(),
            KeyCode::Enter => self.open_edit(),
            KeyCode::Char('d') | KeyCode::Delete => self.open_delete(),
            KeyCode::Char('m') => self.open_move_note(),
            KeyCode::Char('o') => self.open_image(),
            code => {
                let delta = nav_delta(code);
                if delta != 0 {
                    self.move_selection(delta);
                }
            }
        }
    }

    fn move_selection(&mut self, delta: isize) {
        match self.focus {
            Focus::Courses => {
                if move_state(&mut self.course_state, self.courses.len(), delta) {
                    self.chapter_state.select(None);
                    self.note_state.select(None);
                    self.defer("Loading chapters…", |app| app.show_chapters(None));
                }
            }
            Focus::Chapters => {
                if move_state(&mut self.chapter_state, self.chapters.len(), delta) {
                    self.note_state.select(None);
                    self.defer("Loading notes…", |app| app.show_notes(None));
                }
            }
            Focus::Notes => {
                if move_state(&mut self.note_state, self.notes.len(), delta) {
                    self.image_state.select(None);
                    self.reset_images();
                }
            }
            Focus::Images => {
                let len = self.selected_note().map_or(0, |n| n.images.len());
                move_state(&mut self.image_state, len, delta);
            }
        }
    }

    // ---- Opening modals ------------------------------------------------------

    fn open_add(&mut self) {
        let (title, fields, action) = match self.focus {
            Focus::Courses => ("New course".to_owned(), course_fields("", "", ""), FormAction::NewCourse),
            Focus::Chapters => {
                let Some(course) = self.selected_course() else {
                    return self.fail("Add a course first");
                };
                (
                    format!("New chapter in {}", course.name),
                    chapter_fields("", ""),
                    FormAction::NewChapter { course_slug: course.slug.clone() },
                )
            }
            Focus::Notes => {
                let Some(chapter) = self.selected_chapter() else {
                    return self.fail("Add a chapter first");
                };
                let mut fields = note_fields("", "", "");
                fields.push(images_field());
                (format!("New note in {}", chapter.title), fields, FormAction::NewNote {
                    chapter_id: chapter.id.clone(),
                })
            }
            Focus::Images => return self.open_add_images(),
        };
        self.modal = Some(Modal::Form { title, form: Form::new(fields), action });
    }

    fn open_add_images(&mut self) {
        let Some(note) = self.selected_note() else {
            return self.fail("Select a note first");
        };
        self.modal = Some(Modal::Form {
            title: format!("Add images to {}", note.note.title),
            form: Form::new(vec![images_field()]),
            action: FormAction::AddImages { note_id: note.note.id.clone() },
        });
    }

    fn open_edit(&mut self) {
        let modal = match self.focus {
            Focus::Courses => self.selected_course().map(|c| Modal::Form {
                title: format!("Edit course {}", c.name),
                form: Form::new(course_fields(&c.slug, &c.name, &c.description)),
                action: FormAction::EditCourse { slug: c.slug.clone() },
            }),
            Focus::Chapters => self.selected_chapter().map(|c| Modal::Form {
                title: format!("Edit chapter {}", c.title),
                form: Form::new(chapter_fields(&c.title, &c.position.to_string())),
                action: FormAction::EditChapter { id: c.id.clone() },
            }),
            Focus::Notes | Focus::Images => self.selected_note().map(|n| Modal::Form {
                title: format!("Edit note {}", n.note.title),
                form: Form::new(note_fields(
                    &n.note.title,
                    n.note.author_name.as_deref().unwrap_or_default(),
                    &n.note.body,
                )),
                action: FormAction::EditNote { id: n.note.id.clone() },
            }),
        };
        match modal {
            Some(m) => self.modal = Some(m),
            None => self.fail("Nothing selected"),
        }
    }

    fn open_delete(&mut self) {
        let modal = match self.focus {
            Focus::Courses => self.selected_course().map(|c| Modal::Confirm {
                message: format!(
                    "Delete course \"{}\" ({})?\n\nThis permanently deletes ALL of its chapters, notes and images.",
                    c.name, c.slug
                ),
                action: DeleteTarget::Course { id: c.id.clone(), slug: c.slug.clone() },
            }),
            Focus::Chapters => self.selected_chapter().map(|c| {
                let notes = self
                    .cached_note_count(&c.id)
                    .map_or("all of its".to_owned(), |n| format!("its {n}"));
                Modal::Confirm {
                    message: format!(
                        "Delete chapter \"{}\"?\n\nThis permanently deletes {notes} notes and their images.",
                        c.title
                    ),
                    action: DeleteTarget::Chapter { id: c.id.clone() },
                }
            }),
            Focus::Notes => self.selected_note().map(|n| Modal::Confirm {
                message: format!(
                    "Delete note \"{}\" and its {} image(s)?",
                    n.note.title,
                    n.images.len()
                ),
                action: DeleteTarget::Note { id: n.note.id.clone() },
            }),
            Focus::Images => {
                let note = self.selected_note();
                let index = self.image_state.selected();
                self.selected_image().zip(note).zip(index).map(|((img, note), i)| Modal::Confirm {
                    message: format!(
                        "Delete image {} ({}) from \"{}\"?",
                        i + 1,
                        img.original_filename.as_deref().unwrap_or("unnamed"),
                        note.note.title
                    ),
                    action: DeleteTarget::Image {
                        note_id: note.note.id.clone(),
                        image_id: img.id.clone(),
                    },
                })
            }
        };
        match modal {
            Some(m) => self.modal = Some(m),
            None => self.fail("Nothing selected"),
        }
    }

    fn open_move_note(&mut self) {
        let Some(note) = self.selected_note() else {
            return self.fail("Select a note to move");
        };
        let (note_id, from_chapter) = (note.note.id.clone(), note.note.chapter_id.clone());
        self.defer("Loading chapters…", move |app| {
            let api = app.api();
            let mut items = Vec::new();
            for course in app.courses.clone() {
                if !app.chapters_cache.contains_key(&course.id) {
                    match app.rt.block_on(api.course(&course.slug)) {
                        Ok(d) => {
                            let chapters = d.chapters.into_iter().map(|c| c.chapter).collect();
                            app.chapters_cache.insert(course.id.clone(), chapters);
                        }
                        Err(e) => return app.fail(e),
                    }
                }
                for ch in &app.chapters_cache[&course.id] {
                    items.push(PickItem {
                        label: format!("{} › {}", course.name, ch.title),
                        chapter_id: ch.id.clone(),
                        current: ch.id == from_chapter,
                    });
                }
            }
            let mut state = ListState::default();
            state.select(items.iter().position(|i| i.current).or(Some(0)));
            app.modal = Some(Modal::MoveNote { note_id, from_chapter, items, state });
        });
    }

    fn open_image(&mut self) {
        let url = self
            .selected_image()
            .or_else(|| self.selected_note().and_then(|n| n.images.first()))
            .map(|i| i.url.clone());
        match url {
            Some(url) => match open_in_browser(&url) {
                Ok(()) => self.info(format!("Opened {url}")),
                Err(e) => self.fail(format!("could not open browser: {e}")),
            },
            None => self.fail("No image selected"),
        }
    }

    // ---- Actions -------------------------------------------------------------

    fn submit_form(&mut self, title: String, form: Form, action: FormAction) {
        self.defer("Saving…", move |app| {
            if let Err(e) = app.execute(&form, &action) {
                // Keep the form open so the input can be corrected.
                app.modal = Some(Modal::Form { title, form, action });
                app.fail(e);
            }
        });
    }

    fn execute(&mut self, form: &Form, action: &FormAction) -> Result<(), String> {
        let api = self.api();
        let rt = &self.rt;
        let text = |i: usize| form.value(i).trim().to_owned();
        match action {
            FormAction::NewCourse => {
                let req = CreateCourse { slug: text(0), name: text(1), description: text(2) };
                let course = rt.block_on(api.create_course(&req)).map_err(|e| e.to_string())?;
                self.focus = Focus::Courses;
                self.load_courses(Some(course.id));
                self.info(format!("Created course {}", req.name));
            }
            FormAction::EditCourse { slug } => {
                let req = UpdateCourse {
                    slug: Some(text(0)),
                    name: Some(text(1)),
                    description: Some(text(2)),
                };
                let course = rt.block_on(api.update_course(slug, &req)).map_err(|e| e.to_string())?;
                self.load_courses(Some(course.id));
                self.info(format!("Saved course {}", course.name));
            }
            FormAction::NewChapter { course_slug } => {
                let req = CreateChapter { title: text(0), position: parse_position(&text(1))? };
                let chapter = rt
                    .block_on(api.create_chapter(course_slug, &req))
                    .map_err(|e| e.to_string())?;
                self.focus = Focus::Chapters;
                self.refresh_chapters(Some(chapter.id));
                self.info(format!("Created chapter {}", chapter.title));
            }
            FormAction::EditChapter { id } => {
                let req = UpdateChapter { title: Some(text(0)), position: parse_position(&text(1))? };
                let chapter = rt.block_on(api.update_chapter(id, &req)).map_err(|e| e.to_string())?;
                self.refresh_chapters(Some(chapter.id));
                self.info(format!("Saved chapter {}", chapter.title));
            }
            FormAction::NewNote { chapter_id } => {
                let files = parse_image_paths(form.value(3))?;
                if files.is_empty() {
                    return Err("add at least one image (file paths or a folder)".into());
                }
                let body = form.value(2).to_owned();
                let note = rt
                    .block_on(api.create_note(chapter_id, &text(0), &body, &text(1), &files))
                    .map_err(|e| e.to_string())?;
                self.focus = Focus::Notes;
                self.refresh_notes(Some(note.note.id));
                self.info(format!("Uploaded note {} with {} image(s)", note.note.title, note.images.len()));
            }
            FormAction::EditNote { id } => {
                let req = UpdateNote {
                    title: Some(text(0)),
                    author_name: Some(text(1)),
                    body: Some(form.value(2).to_owned()),
                    chapter_id: None,
                };
                let note = rt.block_on(api.update_note(id, &req)).map_err(|e| e.to_string())?;
                self.refresh_notes(Some(note.note.id));
                self.info(format!("Saved note {}", note.note.title));
            }
            FormAction::AddImages { note_id } => {
                let files = parse_image_paths(form.value(0))?;
                if files.is_empty() {
                    return Err("enter at least one image path or a folder".into());
                }
                let added = rt.block_on(api.add_images(note_id, &files)).map_err(|e| e.to_string())?;
                self.refresh_notes(Some(note_id.clone()));
                self.info(format!("Added {} image(s)", added.len()));
            }
        }
        Ok(())
    }

    fn confirm(&mut self, action: DeleteTarget) {
        let api = self.api();
        let result = match &action {
            DeleteTarget::Course { slug, .. } => self.rt.block_on(api.delete_course(slug)),
            DeleteTarget::Chapter { id } => self.rt.block_on(api.delete_chapter(id)),
            DeleteTarget::Note { id } => self.rt.block_on(api.delete_note(id)),
            DeleteTarget::Image { note_id, image_id } => {
                self.rt.block_on(api.delete_image(note_id, image_id))
            }
        };
        if let Err(e) = result {
            return self.fail(e);
        }
        match action {
            DeleteTarget::Course { id, .. } => {
                self.chapters_cache.remove(&id);
                self.load_courses(None);
                self.info("Course deleted");
            }
            DeleteTarget::Chapter { id } => {
                self.notes_cache.remove(&id);
                self.refresh_chapters(None);
                self.info("Chapter deleted");
            }
            DeleteTarget::Note { .. } => {
                self.refresh_notes(None);
                self.info("Note deleted");
            }
            DeleteTarget::Image { note_id, .. } => {
                self.refresh_notes(Some(note_id));
                self.info("Image deleted");
            }
        }
    }

    fn move_note(&mut self, note_id: String, from: String, to: String) {
        let api = self.api();
        let req = UpdateNote { chapter_id: Some(to.clone()), ..Default::default() };
        if let Err(e) = self.rt.block_on(api.update_note(&note_id, &req)) {
            return self.fail(e);
        }
        self.notes_cache.remove(&from);
        self.notes_cache.remove(&to);
        self.show_notes(None);
        self.info("Note moved");
    }

    /// Swaps the selected chapter with its neighbour and renumbers positions 0..n.
    fn reorder_chapter(&mut self, delta: isize) {
        if self.focus != Focus::Chapters {
            return self.fail("Reorder chapters with Shift+↑/↓ (or K/J) in the Chapters column");
        }
        let Some(i) = self.chapter_state.selected() else { return };
        let j = i as isize + delta;
        if j < 0 || j as usize >= self.chapters.len() {
            return;
        }
        let mut order = self.chapters.clone();
        order.swap(i, j as usize);
        let moved = order[j as usize].id.clone();
        self.defer("Reordering…", move |app| {
            let api = app.api();
            for (pos, ch) in order.iter().enumerate() {
                if ch.position != pos as i64 {
                    let req = UpdateChapter { title: None, position: Some(pos as i64) };
                    if let Err(e) = app.rt.block_on(api.update_chapter(&ch.id, &req)) {
                        app.fail(e);
                        break;
                    }
                }
            }
            app.refresh_chapters(Some(moved));
        });
    }

    fn refresh_chapters(&mut self, keep: Option<String>) {
        if let Some(course) = self.selected_course() {
            let id = course.id.clone();
            self.chapters_cache.remove(&id);
        }
        self.show_chapters(keep);
    }

    fn refresh_notes(&mut self, keep: Option<String>) {
        if let Some(chapter) = self.selected_chapter() {
            let id = chapter.id.clone();
            self.notes_cache.remove(&id);
        }
        self.show_notes(keep);
    }
}

// ---- Helpers -----------------------------------------------------------------

fn course_fields(slug: &str, name: &str, description: &str) -> Vec<Field> {
    vec![
        Field::new("Slug", FieldKind::Line, slug).hint("URL name: a-z, 0-9 and -"),
        Field::new("Name", FieldKind::Line, name),
        Field::new("Description", FieldKind::Multiline, description),
    ]
}

fn chapter_fields(title: &str, position: &str) -> Vec<Field> {
    vec![
        Field::new("Title", FieldKind::Line, title),
        Field::new("Position", FieldKind::Line, position).hint("0 = first; empty = last"),
    ]
}

fn note_fields(title: &str, author: &str, body: &str) -> Vec<Field> {
    vec![
        Field::new("Title", FieldKind::Line, title),
        Field::new("Author", FieldKind::Line, author).hint("optional"),
        Field::new("Text", FieldKind::Multiline, body).hint("Enter = new line, Ctrl+S = save"),
    ]
}

fn images_field() -> Field {
    Field::new("Images", FieldKind::Line, "")
        .hint("file paths or a folder, separated by ; (drag & drop works)")
}

fn parse_position(s: &str) -> Result<Option<i64>, String> {
    if s.is_empty() {
        return Ok(None);
    }
    s.parse::<i64>()
        .ok()
        .filter(|p| *p >= 0)
        .map(Some)
        .ok_or_else(|| format!("position must be a whole number >= 0, got \"{s}\""))
}

fn nav_delta(code: KeyCode) -> isize {
    match code {
        KeyCode::Up | KeyCode::Char('k') => -1,
        KeyCode::Down | KeyCode::Char('j') => 1,
        KeyCode::PageUp => -10,
        KeyCode::PageDown => 10,
        KeyCode::Home => isize::MIN / 2,
        KeyCode::End => isize::MAX / 2,
        _ => 0,
    }
}

/// Moves a list selection, clamped. Returns whether it changed.
fn move_state(state: &mut ListState, len: usize, delta: isize) -> bool {
    if len == 0 || delta == 0 {
        return false;
    }
    let current = state.selected().unwrap_or(0) as isize;
    let next = current.saturating_add(delta).clamp(0, len as isize - 1) as usize;
    let changed = state.selected() != Some(next);
    state.select(Some(next));
    changed
}

/// Selects `keep` if present, else the previous index clamped, else the first item.
fn select_by<T>(state: &mut ListState, items: &[T], keep: Option<&str>, id: impl Fn(&T) -> &String) {
    if items.is_empty() {
        state.select(None);
        return;
    }
    let index = keep
        .and_then(|k| items.iter().position(|item| id(item) == k))
        .or_else(|| state.selected().map(|i| i.min(items.len() - 1)))
        .unwrap_or(0);
    state.select(Some(index));
}

fn open_in_browser(url: &str) -> std::io::Result<()> {
    use std::process::{Command, Stdio};
    #[cfg(windows)]
    let mut cmd = {
        let mut c = Command::new("cmd");
        c.args(["/C", "start", "", url]);
        c
    };
    #[cfg(target_os = "macos")]
    let mut cmd = {
        let mut c = Command::new("open");
        c.arg(url);
        c
    };
    #[cfg(all(unix, not(target_os = "macos")))]
    let mut cmd = {
        let mut c = Command::new("xdg-open");
        c.arg(url);
        c
    };
    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(drop)
}
