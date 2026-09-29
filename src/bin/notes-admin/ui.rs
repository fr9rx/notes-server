use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, BorderType, Clear, List, ListItem, ListState, Paragraph, Wrap};

use crate::app::{App, Focus, Modal, StatusKind};
use crate::input::{FieldKind, Form};

const ACCENT: Color = Color::Cyan;

pub fn draw(frame: &mut Frame, app: &mut App) {
    let [header, body, footer] =
        Layout::vertical([Constraint::Length(1), Constraint::Min(0), Constraint::Length(1)])
            .areas(frame.area());

    let target = app
        .api
        .as_ref()
        .map_or_else(|| "not connected".to_owned(), |a| a.base().to_owned());
    frame.render_widget(
        Line::from(vec![
            " notes-admin ".bold().black().on_cyan(),
            Span::raw(" "),
            Span::styled(target, Style::new().fg(Color::Gray)),
        ]),
        header,
    );

    if app.api.is_none() {
        draw_login(frame, body, app);
    } else {
        draw_browser(frame, body, app);
    }
    draw_footer(frame, footer, app);

    if let Some(modal) = app.modal.as_mut() {
        draw_modal(frame, body, modal);
    }
}

fn draw_login(frame: &mut Frame, area: Rect, app: &mut App) {
    let height = form_height(&app.login) + 4;
    let rect = centered(area, 72, height);
    frame.render_widget(Clear, rect);
    let block = modal_block("Connect to notes-server", ACCENT);
    let inner = block.inner(rect);
    frame.render_widget(block, rect);
    let [fields, help] =
        Layout::vertical([Constraint::Min(0), Constraint::Length(2)]).areas(inner);
    draw_form_fields(frame, fields, &app.login);
    frame.render_widget(
        Paragraph::new(
            "Tip: set NOTES_URL and NOTES_ADMIN_TOKEN (and NOTES_CA_CERT) to skip this screen.",
        )
        .style(Style::new().fg(Color::DarkGray))
        .wrap(Wrap { trim: true }),
        help,
    );
}

fn draw_browser(frame: &mut Frame, area: Rect, app: &mut App) {
    let [courses_area, chapters_area, right] = Layout::horizontal([
        Constraint::Percentage(22),
        Constraint::Percentage(26),
        Constraint::Min(30),
    ])
    .areas(area);
    let images_height = app
        .selected_note()
        .map_or(3, |n| (n.images.len() as u16 + 2).clamp(3, 10));
    let [notes_area, detail_area, images_area] = Layout::vertical([
        Constraint::Percentage(40),
        Constraint::Min(5),
        Constraint::Length(images_height),
    ])
    .areas(right);

    // Courses
    let items: Vec<ListItem> = app
        .courses
        .iter()
        .map(|c| {
            let mut line = Line::from(vec![
                Span::raw(c.course.name.clone()),
                Span::styled(format!("  {}", c.course.slug), Style::new().fg(Color::DarkGray)),
            ]);
            if c.custom_cover {
                line.push_span(Span::styled("  ▣", Style::new().fg(ACCENT)));
            }
            ListItem::new(line)
        })
        .collect();
    draw_list(
        frame,
        courses_area,
        "Courses",
        items,
        &mut app.course_state,
        app.focus == Focus::Courses,
        "No courses. Press a to add one.",
    );

    // Chapters
    let items: Vec<ListItem> = app
        .chapters
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let count = app
                .cached_note_count(&c.id)
                .map(|n| format!("  ({n})"))
                .unwrap_or_default();
            ListItem::new(Line::from(vec![
                Span::styled(format!("{:>2}. ", i + 1), Style::new().fg(Color::DarkGray)),
                Span::raw(c.title.clone()),
                Span::styled(count, Style::new().fg(Color::DarkGray)),
            ]))
        })
        .collect();
    let chapters_title = app
        .selected_course()
        .map_or("Chapters".to_owned(), |c| format!("Chapters · {}", c.name));
    let chapters_hint = if app.selected_course().is_some() { "No chapters. Press a to add one." } else { "" };
    draw_list(
        frame,
        chapters_area,
        &chapters_title,
        items,
        &mut app.chapter_state,
        app.focus == Focus::Chapters,
        chapters_hint,
    );

    // Notes
    let items: Vec<ListItem> = app
        .notes
        .iter()
        .map(|n| {
            let mut meta = vec![format!("{} img", n.images.len())];
            if let Some(author) = &n.note.author_name {
                meta.insert(0, author.clone());
            }
            meta.push(n.note.created_at.chars().take(10).collect());
            ListItem::new(Line::from(vec![
                Span::raw(n.note.title.clone()),
                Span::styled(format!("  {}", meta.join(" · ")), Style::new().fg(Color::DarkGray)),
            ]))
        })
        .collect();
    let notes_title = app
        .selected_chapter()
        .map_or("Notes".to_owned(), |c| format!("Notes · {}", c.title));
    let notes_hint = if app.selected_chapter().is_some() { "No notes. Press a to upload one." } else { "" };
    draw_list(
        frame,
        notes_area,
        &notes_title,
        items,
        &mut app.note_state,
        app.focus == Focus::Notes,
        notes_hint,
    );

    // Note detail
    let detail = match app.selected_note() {
        Some(n) => {
            let mut lines = vec![
                Line::from(n.note.title.clone().bold()),
                Line::from(Span::styled(
                    format!(
                        "by {} · created {} · updated {}",
                        n.note.author_name.as_deref().unwrap_or("anonymous"),
                        short_time(&n.note.created_at),
                        short_time(&n.note.updated_at),
                    ),
                    Style::new().fg(Color::DarkGray),
                )),
                Line::default(),
            ];
            if n.note.body.is_empty() {
                lines.push(Line::from("(no text)".italic().dark_gray()));
            } else {
                lines.extend(n.note.body.lines().map(|l| Line::from(l.to_owned())));
            }
            Text::from(lines)
        }
        None => Text::from(""),
    };
    frame.render_widget(
        Paragraph::new(detail)
            .wrap(Wrap { trim: false })
            .block(pane_block("Note", false)),
        detail_area,
    );

    // Images
    let images: Vec<ListItem> = app
        .selected_note()
        .map(|n| {
            n.images
                .iter()
                .enumerate()
                .map(|(i, img)| {
                    ListItem::new(Line::from(vec![
                        Span::styled(format!("{:>2}. ", i + 1), Style::new().fg(Color::DarkGray)),
                        Span::raw(img.original_filename.clone().unwrap_or_else(|| "unnamed".into())),
                        Span::styled(
                            format!(
                                "  {}×{}  {}",
                                img.width,
                                img.height,
                                human_bytes(img.size_bytes)
                            ),
                            Style::new().fg(Color::DarkGray),
                        ),
                    ]))
                })
                .collect()
        })
        .unwrap_or_default();
    let images_hint = if app.selected_note().is_some() { "No images. Press i to add some." } else { "" };
    draw_list(
        frame,
        images_area,
        "Images",
        images,
        &mut app.image_state,
        app.focus == Focus::Images,
        images_hint,
    );
}

fn draw_list(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    items: Vec<ListItem>,
    state: &mut ListState,
    focused: bool,
    empty_hint: &str,
) {
    let block = pane_block(title, focused);
    if items.is_empty() {
        frame.render_widget(
            Paragraph::new(empty_hint.italic().dark_gray())
                .wrap(Wrap { trim: true })
                .block(block),
            area,
        );
        return;
    }
    let highlight = if focused {
        Style::new().bg(ACCENT).fg(Color::Black).add_modifier(Modifier::BOLD)
    } else {
        Style::new().add_modifier(Modifier::REVERSED)
    };
    let list = List::new(items)
        .block(block)
        .highlight_style(highlight)
        .highlight_symbol(if focused { "▶ " } else { "  " });
    frame.render_stateful_widget(list, area, state);
}

fn pane_block(title: &str, focused: bool) -> Block<'static> {
    let (border, title_style) = if focused {
        (Style::new().fg(ACCENT), Style::new().fg(ACCENT).add_modifier(Modifier::BOLD))
    } else {
        (Style::new().fg(Color::DarkGray), Style::new())
    };
    Block::bordered()
        .border_type(if focused { BorderType::Thick } else { BorderType::Rounded })
        .border_style(border)
        .title(Span::styled(format!(" {title} "), title_style))
}

fn draw_footer(frame: &mut Frame, area: Rect, app: &App) {
    let line = match &app.status {
        Some((StatusKind::Busy, msg)) => Line::from(format!(" ⟳ {msg}").yellow().bold()),
        Some((StatusKind::Error, msg)) => Line::from(format!(" ✗ {msg}").red().bold()),
        Some((StatusKind::Info, msg)) => Line::from(format!(" ✓ {msg}").green()),
        None => hints(app),
    };
    frame.render_widget(line, area);
}

fn hints(app: &App) -> Line<'static> {
    let keys: &[(&str, &str)] = if app.api.is_none() {
        &[("Tab", "next field"), ("Enter", "connect"), ("Esc", "quit")]
    } else {
        match &app.modal {
            Some(Modal::Form { .. }) => &[
                ("Tab/↑↓", "field"),
                ("Enter", "next/save"),
                ("Ctrl+S", "save"),
                ("Ctrl+U", "clear"),
                ("Esc", "cancel"),
            ],
            Some(Modal::Confirm { .. }) => &[("y", "yes, delete"), ("n/Esc", "cancel")],
            Some(Modal::MoveNote { .. }) => &[("↑↓", "choose"), ("Enter", "move"), ("Esc", "cancel")],
            Some(Modal::Help) => &[("any key", "close")],
            None => match app.focus {
                Focus::Courses => &[
                    ("←→", "column"),
                    ("a", "add"),
                    ("e", "edit"),
                    ("d", "delete"),
                    ("c/C", "set/remove cover"),
                    ("o", "open cover"),
                    ("?", "help"),
                    ("q", "quit"),
                ],
                Focus::Chapters => &[
                    ("←→", "column"),
                    ("a", "add"),
                    ("e", "edit"),
                    ("d", "delete"),
                    ("Shift+↑↓", "reorder"),
                    ("?", "help"),
                ],
                Focus::Notes => &[
                    ("a", "upload"),
                    ("e", "edit"),
                    ("d", "delete"),
                    ("m", "move"),
                    ("i", "add images"),
                    ("o", "open"),
                    ("?", "help"),
                ],
                Focus::Images => &[
                    ("i", "add images"),
                    ("d", "delete image"),
                    ("Enter/o", "open in browser"),
                    ("?", "help"),
                ],
            },
        }
    };
    let mut spans = vec![Span::raw(" ")];
    for (key, what) in keys {
        spans.push(Span::styled((*key).to_owned(), Style::new().fg(ACCENT).bold()));
        spans.push(Span::styled(format!(" {what}   "), Style::new().fg(Color::Gray)));
    }
    Line::from(spans)
}

fn draw_modal(frame: &mut Frame, area: Rect, modal: &mut Modal) {
    match modal {
        Modal::Form { title, form, .. } => {
            let rect = centered(area, 76, form_height(form) + 2);
            frame.render_widget(Clear, rect);
            let block = modal_block(title, ACCENT);
            let inner = block.inner(rect);
            frame.render_widget(block, rect);
            draw_form_fields(frame, inner, form);
        }
        Modal::Confirm { message, .. } => {
            let rect = centered(area, 64, 9);
            frame.render_widget(Clear, rect);
            let text = Text::from(vec![
                Line::from(message.clone()),
                Line::default(),
                Line::from(vec![
                    "y".red().bold(),
                    " delete    ".into(),
                    "n".bold(),
                    " cancel".into(),
                ]),
            ]);
            frame.render_widget(
                Paragraph::new(text)
                    .wrap(Wrap { trim: false })
                    .block(modal_block("Confirm delete", Color::Red)),
                rect,
            );
        }
        Modal::MoveNote { items, state, .. } => {
            let rect = centered(area, 64, (items.len() as u16 + 2).clamp(5, 20));
            frame.render_widget(Clear, rect);
            let list = List::new(items.iter().map(|i| {
                let mut line = Line::from(i.label.clone());
                if i.current {
                    line.push_span(Span::styled("  (current)", Style::new().fg(Color::DarkGray)));
                }
                ListItem::new(line)
            }))
            .block(modal_block("Move note to…", ACCENT))
            .highlight_style(Style::new().bg(ACCENT).fg(Color::Black).bold())
            .highlight_symbol("▶ ");
            frame.render_stateful_widget(list, rect, state);
        }
        Modal::Help => {
            let rect = centered(area, 70, 24);
            frame.render_widget(Clear, rect);
            let rows = [
                ("←/→, Tab, h/l", "switch column (Courses, Chapters, Notes, Images)"),
                ("↑/↓, j/k, PgUp/PgDn", "move selection"),
                ("a / n", "add a course, chapter or note (in the focused column)"),
                ("e / Enter", "edit the selected item"),
                ("d / Del", "delete the selected item (asks first)"),
                ("Shift+↑/↓, K/J", "reorder chapters"),
                ("m", "move the selected note to another chapter/course"),
                ("i", "add images to the selected note"),
                ("c / C", "set / remove the selected course's cover photo (▣)"),
                ("o / Enter on image", "open the image (or course cover) in your browser"),
                ("r / F5", "reload everything from the server"),
                ("q, Ctrl+C", "quit"),
                ("", ""),
                ("Images", "enter file paths or a folder, separated by ;"),
                ("", "drag files onto the terminal to paste their paths"),
                ("Text fields", "Enter = new line, Ctrl+S = save"),
            ];
            let lines: Vec<Line> = rows
                .iter()
                .map(|(k, v)| {
                    Line::from(vec![
                        Span::styled(format!("{k:<22}"), Style::new().fg(ACCENT).bold()),
                        Span::raw(*v),
                    ])
                })
                .collect();
            frame.render_widget(
                Paragraph::new(lines)
                    .wrap(Wrap { trim: false })
                    .block(modal_block("Keys", ACCENT)),
                rect,
            );
        }
    }
}

fn field_height(kind: FieldKind) -> u16 {
    match kind {
        FieldKind::Multiline => 7,
        _ => 3,
    }
}

fn form_height(form: &Form) -> u16 {
    form.fields.iter().map(|f| field_height(f.kind)).sum()
}

fn draw_form_fields(frame: &mut Frame, area: Rect, form: &Form) {
    let areas = Layout::vertical(
        form.fields
            .iter()
            .map(|f| Constraint::Length(field_height(f.kind))),
    )
    .split(area);

    for (i, (field, rect)) in form.fields.iter().zip(areas.iter()).enumerate() {
        let active = i == form.active;
        let mut block = Block::bordered()
            .border_type(BorderType::Rounded)
            .border_style(if active { Style::new().fg(ACCENT) } else { Style::new().fg(Color::DarkGray) })
            .title(Span::styled(
                format!(" {} ", field.label),
                if active { Style::new().fg(ACCENT).bold() } else { Style::new() },
            ));
        if !field.hint.is_empty() {
            block = block.title_bottom(Line::from(Span::styled(
                format!(" {} ", field.hint),
                Style::new().fg(Color::DarkGray),
            )));
        }
        let inner = block.inner(*rect);

        // Scroll so the cursor stays visible.
        let (line, col) = field.cursor_line_col();
        let scroll_y = line.saturating_sub(inner.height.saturating_sub(1) as usize) as u16;
        let scroll_x = col.saturating_sub(inner.width.saturating_sub(1) as usize) as u16;
        frame.render_widget(
            Paragraph::new(field.display()).scroll((scroll_y, scroll_x)).block(block),
            *rect,
        );
        if active && inner.width > 0 && inner.height > 0 {
            frame.set_cursor_position((
                inner.x + (col as u16).saturating_sub(scroll_x),
                inner.y + (line as u16).saturating_sub(scroll_y),
            ));
        }
    }
}

fn modal_block(title: &str, color: Color) -> Block<'static> {
    Block::bordered()
        .border_type(BorderType::Double)
        .border_style(Style::new().fg(color))
        .title(Span::styled(format!(" {title} "), Style::new().fg(color).bold()))
}

fn centered(area: Rect, width: u16, height: u16) -> Rect {
    let w = width.min(area.width);
    let h = height.min(area.height);
    Rect {
        x: area.x + (area.width - w) / 2,
        y: area.y + (area.height - h) / 2,
        width: w,
        height: h,
    }
}

/// `2026-09-27T18:18:20.788Z` -> `2026-09-27 18:18`
fn short_time(ts: &str) -> String {
    ts.get(..16).map_or_else(|| ts.to_owned(), |s| s.replace('T', " "))
}

fn human_bytes(n: i64) -> String {
    match n {
        n if n >= 1024 * 1024 => format!("{:.1} MB", n as f64 / (1024.0 * 1024.0)),
        n if n >= 1024 => format!("{} KB", n / 1024),
        n => format!("{n} B"),
    }
}
