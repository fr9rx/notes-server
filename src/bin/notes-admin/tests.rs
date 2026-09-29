//! Drives the real TUI (state + rendering) with key presses against a real
//! notes-server running in a background thread.

use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use notes_server::config::AppSettings;
use notes_server::{AppState, db};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tempfile::TempDir;

use crate::app::{App, Focus, Modal, StatusKind};

const TOKEN: &str = "tui-test-admin-token-0123456789abcdef";

/// Starts a plain-HTTP server on a random port; returns its URL.
fn spawn_server(dir: &Path) -> String {
    let dir = dir.to_owned();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async move {
            let url = format!("sqlite://{}", dir.join("t.db").display().to_string().replace('\\', "/"));
            let pool = db::connect(&url).await.unwrap();
            db::migrate(&pool).await.unwrap();
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let addr = listener.local_addr().unwrap();
            let mut settings = AppSettings::new(TOKEN, format!("http://{addr}"));
            settings.min_free_disk_bytes = 0;
            settings.upload_burst = 1000;
            let state = AppState::new(pool, dir.join("uploads"), settings).unwrap();
            tx.send(addr).unwrap();
            axum::serve(
                listener,
                notes_server::app(state).into_make_service_with_connect_info::<SocketAddr>(),
            )
            .await
            .unwrap();
        });
    });
    format!("http://{}", rx.recv().unwrap())
}

struct Harness {
    app: App,
    term: Terminal<TestBackend>,
    _dir: TempDir,
    files: PathBuf,
}

impl Harness {
    fn new(token: &str) -> Self {
        notes_server::tls::install_crypto_provider();
        let dir = tempfile::tempdir().unwrap();
        let url = spawn_server(dir.path());
        let files = dir.path().join("photos");
        std::fs::create_dir_all(&files).unwrap();
        for (name, w, h) in [("a.jpg", 2000u16, 1000u16), ("b.jpg", 640, 480), ("c.jpg", 300, 300)] {
            std::fs::write(files.join(name), jpeg(w, h)).unwrap();
        }
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        let app = App::new(rt, url, token.to_owned(), None, false);
        let mut h = Self { app, term: Terminal::new(TestBackend::new(140, 40)).unwrap(), _dir: dir, files };
        h.settle();
        h
    }

    /// Runs queued work and renders, like the real main loop.
    fn settle(&mut self) {
        loop {
            self.term.draw(|f| crate::ui::draw(f, &mut self.app)).unwrap();
            if !self.app.run_pending() {
                break;
            }
        }
    }

    fn key_mod(&mut self, code: KeyCode, modifiers: KeyModifiers) {
        self.app.on_key(KeyEvent::new(code, modifiers));
        self.settle();
    }

    fn key(&mut self, code: KeyCode) {
        self.key_mod(code, KeyModifiers::NONE);
    }

    fn ch(&mut self, c: char) {
        self.key(KeyCode::Char(c));
    }

    fn typ(&mut self, text: &str) {
        for c in text.chars() {
            self.ch(c);
        }
    }

    fn save(&mut self) {
        self.key_mod(KeyCode::Char('s'), KeyModifiers::CONTROL);
    }

    fn screen(&self) -> String {
        let buf = self.term.backend().buffer();
        let mut s = String::new();
        for y in 0..buf.area.height {
            for x in 0..buf.area.width {
                s.push_str(buf[(x, y)].symbol());
            }
            s.push('\n');
        }
        s
    }

    fn assert_ok(&self) {
        if let Some((StatusKind::Error, msg)) = &self.app.status {
            panic!("unexpected error: {msg}\n{}", self.screen());
        }
    }

    fn photo(&self, name: &str) -> String {
        self.files.join(name).display().to_string()
    }
}

fn jpeg(width: u16, height: u16) -> Vec<u8> {
    let rgb: Vec<u8> = (0..width as usize * height as usize)
        .flat_map(|i| [(i % 251) as u8, (i % 241) as u8, 90])
        .collect();
    let mut out = Vec::new();
    jpeg_encoder::Encoder::new(&mut out, 85)
        .encode(&rgb, width, height, jpeg_encoder::ColorType::Rgb)
        .unwrap();
    out
}

#[test]
fn wrong_token_stays_on_login_screen() {
    let h = Harness::new("wrong-token");
    assert!(h.app.api.is_none());
    assert!(matches!(&h.app.status, Some((StatusKind::Error, m)) if m.contains("admin token rejected")));
    assert!(h.screen().contains("Connect to notes-server"));
}

#[test]
fn manage_everything_from_the_tui() {
    let mut h = Harness::new(TOKEN);
    assert!(h.app.api.is_some(), "auto-connect failed: {:?}", h.app.status);
    assert!(h.screen().contains("No courses."));

    // --- Course: add
    h.ch('a');
    assert!(matches!(h.app.modal, Some(Modal::Form { .. })));
    h.typ("math-101");
    h.key(KeyCode::Tab);
    h.typ("Math 101");
    h.save();
    h.assert_ok();
    assert_eq!(h.app.courses.len(), 1);
    assert!(h.screen().contains("Math 101"));

    // Validation error from the server keeps the form open with the message.
    h.ch('a');
    h.typ("Bad Slug");
    h.key(KeyCode::Tab);
    h.typ("x");
    h.save();
    assert!(matches!(h.app.modal, Some(Modal::Form { .. })));
    assert!(matches!(&h.app.status, Some((StatusKind::Error, m)) if m.contains("slug")));
    h.key(KeyCode::Esc);
    assert!(h.app.modal.is_none());

    // --- Chapters: add two, reorder
    h.key(KeyCode::Right);
    assert_eq!(h.app.focus, Focus::Chapters);
    for title in ["Limits", "Derivatives"] {
        h.ch('a');
        h.typ(title);
        h.key(KeyCode::Enter); // next field (position)
        h.key(KeyCode::Enter); // submit
        h.assert_ok();
    }
    let titles = |h: &Harness| h.app.chapters.iter().map(|c| c.title.clone()).collect::<Vec<_>>();
    assert_eq!(titles(&h), ["Limits", "Derivatives"]);
    assert_eq!(h.app.selected_chapter().unwrap().title, "Derivatives");
    h.key_mod(KeyCode::Up, KeyModifiers::SHIFT);
    h.assert_ok();
    assert_eq!(titles(&h), ["Derivatives", "Limits"]);
    assert_eq!(h.app.selected_chapter().unwrap().title, "Derivatives", "selection follows the moved chapter");
    h.ch('j'); // select "Limits"

    // --- Note: upload with a folder of images
    h.key(KeyCode::Right);
    h.ch('a');
    h.typ("Lecture 1");
    h.key(KeyCode::Tab);
    h.typ("Sam");
    h.key(KeyCode::Tab);
    h.typ("Squeeze theorem");
    h.key(KeyCode::Enter); // newline in the text field
    h.typ("and more");
    h.key(KeyCode::Tab);
    let folder = h.files.display().to_string();
    h.app.on_paste(&format!("\"{folder}\""));
    h.save();
    h.assert_ok();
    let note = h.app.selected_note().unwrap().clone();
    assert_eq!(note.note.title, "Lecture 1");
    assert_eq!(note.note.author_name.as_deref(), Some("Sam"));
    assert_eq!(note.note.body, "Squeeze theorem\nand more");
    assert_eq!(note.images.len(), 3);
    assert_eq!((note.images[0].width, note.images[0].height), (1600, 800));
    let screen = h.screen();
    if std::env::var_os("NOTES_ADMIN_DUMP").is_some() {
        println!("{screen}"); // eyeball the layout: NOTES_ADMIN_DUMP=1 cargo test -- --nocapture
    }
    assert!(screen.contains("Lecture 1") && screen.contains("a.jpg") && screen.contains("1600×800"), "{screen}");

    // --- Images: add one, delete one
    h.ch('i');
    let one = h.photo("c.jpg");
    h.typ(&one);
    h.key(KeyCode::Enter);
    h.assert_ok();
    assert_eq!(h.app.selected_note().unwrap().images.len(), 4);
    h.key(KeyCode::Right);
    assert_eq!(h.app.focus, Focus::Images);
    h.ch('d');
    assert!(matches!(h.app.modal, Some(Modal::Confirm { .. })));
    h.ch('n'); // cancel first
    assert_eq!(h.app.selected_note().unwrap().images.len(), 4);
    h.ch('d');
    h.ch('y');
    h.assert_ok();
    let images = &h.app.selected_note().unwrap().images;
    assert_eq!(images.len(), 3);
    assert_eq!(images[0].original_filename.as_deref(), Some("b.jpg"));

    // --- Note: edit
    h.key(KeyCode::Left);
    h.ch('e');
    h.key_mod(KeyCode::Char('u'), KeyModifiers::CONTROL);
    h.typ("Lecture 1 (final)");
    h.save();
    h.assert_ok();
    assert_eq!(h.app.selected_note().unwrap().note.title, "Lecture 1 (final)");

    // --- Note: move to the other chapter
    h.ch('m');
    let Some(Modal::MoveNote { items, .. }) = &h.app.modal else { panic!("no picker") };
    assert_eq!(items.len(), 2);
    h.ch('k'); // "Math 101 › Derivatives" (current is Limits, listed second)
    h.key(KeyCode::Enter);
    h.assert_ok();
    assert!(h.app.notes.is_empty(), "note left the Limits chapter");
    h.key(KeyCode::Left);
    h.ch('k'); // Derivatives
    assert_eq!(h.app.selected_chapter().unwrap().title, "Derivatives");
    assert_eq!(h.app.notes.len(), 1);

    // --- Course: edit, then refresh keeps selections
    h.key(KeyCode::Left);
    h.ch('e');
    h.key(KeyCode::Tab);
    h.key_mod(KeyCode::Char('u'), KeyModifiers::CONTROL);
    h.typ("Calculus I");
    h.save();
    h.assert_ok();
    assert_eq!(h.app.courses[0].name, "Calculus I");
    h.ch('r');
    h.assert_ok();
    assert_eq!(h.app.selected_chapter().unwrap().title, "Derivatives");
    assert_eq!(h.app.selected_note().unwrap().note.title, "Lecture 1 (final)");

    // --- Chapter: delete (with its note)
    h.key(KeyCode::Right);
    h.ch('d');
    let Some(Modal::Confirm { message, .. }) = &h.app.modal else { panic!("no confirm") };
    assert!(message.contains("its 1 notes"), "{message}");
    h.ch('y');
    h.assert_ok();
    assert_eq!(titles(&h), ["Limits"]);

    // --- Course: delete everything
    h.key(KeyCode::Left);
    h.ch('d');
    h.ch('y');
    h.assert_ok();
    assert!(h.app.courses.is_empty() && h.app.chapters.is_empty() && h.app.notes.is_empty());
    assert!(h.screen().contains("No courses."));

    // Help opens and any key closes it.
    h.ch('?');
    assert!(h.screen().contains("reorder chapters"));
    h.ch('x');
    assert!(h.app.modal.is_none());

    h.ch('q');
    assert!(h.app.quit);
}
