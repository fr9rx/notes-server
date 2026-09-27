mod common;

use axum::http::{Method, StatusCode, header};
use common::{ADMIN, BASE_URL, Form, TestApp, jpeg, small_jpeg};
use serde_json::json;

#[tokio::test]
async fn upload_resize_store_and_serve() {
    let t = TestApp::new().await;
    let (slug, chapter_id) = t.course_with_chapter("math-101").await;

    let form = Form::new()
        .text("title", "  Lecture 1: Limits  ")
        .text("body", "Intro to limits")
        .text("author_name", "Sam")
        .file("images", "board.jpg", &jpeg(4000, 2000))
        .file("images", "C:\\Users\\me\\small.jpg", &jpeg(640, 480));
    let r = t
        .call(form.request(&format!("/api/chapters/{chapter_id}/notes"), None))
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.text());
    let note = r.json();
    assert_eq!(note["title"], "Lecture 1: Limits");
    assert_eq!(note["body"], "Intro to limits");
    assert_eq!(note["author_name"], "Sam");
    assert_eq!(note["chapter_id"], chapter_id.as_str());

    let images = note["images"].as_array().unwrap();
    assert_eq!(images.len(), 2);
    // Large image shrunk to fit 1600px, small one left alone; thumbnails fit 300px.
    assert_eq!((images[0]["width"].as_i64(), images[0]["height"].as_i64()), (Some(1600), Some(800)));
    assert_eq!((images[0]["thumb_width"].as_i64(), images[0]["thumb_height"].as_i64()), (Some(300), Some(150)));
    assert_eq!((images[1]["width"].as_i64(), images[1]["height"].as_i64()), (Some(640), Some(480)));
    assert_eq!(images[0]["position"], 0);
    assert_eq!(images[1]["position"], 1);
    assert_eq!(images[0]["original_filename"], "board.jpg");
    assert_eq!(images[1]["original_filename"], "small.jpg"); // path stripped

    // Files are on disk, and served back with long-lived caching.
    for img in images {
        for key in ["url", "thumb_url"] {
            let url = img[key].as_str().unwrap();
            assert!(url.starts_with(&format!("{BASE_URL}/files/notes/")), "{url}");
            let on_disk = std::fs::read(t.file_for_url(url)).unwrap();

            let r = t.get(url.strip_prefix(BASE_URL).unwrap()).await;
            assert_eq!(r.status, StatusCode::OK);
            assert_eq!(r.headers[header::CONTENT_TYPE], "image/jpeg");
            assert!(r.headers[header::CACHE_CONTROL].to_str().unwrap().contains("immutable"));
            assert_eq!(r.headers[header::X_CONTENT_TYPE_OPTIONS], "nosniff");
            assert_eq!(r.body.as_ref(), on_disk.as_slice());
        }
    }
    assert_eq!(images[0]["size_bytes"].as_u64().unwrap() as usize, std::fs::read(t.file_for_url(images[0]["url"].as_str().unwrap())).unwrap().len());

    // Missing files 404 without the immutable cache header.
    let r = t.get("/files/notes/nope/nope.jpg").await;
    assert_eq!(r.status, StatusCode::NOT_FOUND);
    assert!(!r.headers.contains_key(header::CACHE_CONTROL));

    // Readable through the course -> chapter -> note hierarchy.
    let course = t.get(&format!("/api/courses/{slug}")).await.json();
    assert_eq!(course["chapters"][0]["id"], chapter_id.as_str());
    let chapter = t.get(&format!("/api/chapters/{chapter_id}")).await.json();
    assert_eq!(chapter["total_notes"], 1);
    assert_eq!(chapter["notes"][0]["id"], note["id"]);
    assert_eq!(chapter["notes"][0]["images"].as_array().unwrap().len(), 2);
    let fetched = t.get(&format!("/api/notes/{}", note["id"].as_str().unwrap())).await.json();
    assert_eq!(fetched["images"], note["images"]);

    // HSTS on every response.
    assert!(t.get("/health").await.headers.contains_key(header::STRICT_TRANSPORT_SECURITY));
}

#[tokio::test]
async fn courses_list_and_chapter_ordering() {
    let t = TestApp::new().await;
    for (slug, name) in [("phys-1", "Physics"), ("bio-1", "Biology")] {
        let r = t.json(Method::POST, "/api/courses", Some(ADMIN), json!({ "slug": slug, "name": name })).await;
        assert_eq!(r.status, StatusCode::CREATED);
    }
    let list = t.get("/api/courses").await.json();
    let names: Vec<_> = list.as_array().unwrap().iter().map(|c| c["name"].as_str().unwrap()).collect();
    assert_eq!(names, ["Biology", "Physics"]);

    for title in ["Intro", "Kinematics", "Dynamics"] {
        let r = t
            .json(Method::POST, "/api/courses/phys-1/chapters", Some(ADMIN), json!({ "title": title }))
            .await;
        assert_eq!(r.status, StatusCode::CREATED);
    }
    let course = t.get("/api/courses/phys-1").await.json();
    let chapters = course["chapters"].as_array().unwrap();
    let order: Vec<_> = chapters.iter().map(|c| (c["title"].as_str().unwrap(), c["position"].as_i64().unwrap())).collect();
    assert_eq!(order, [("Intro", 0), ("Kinematics", 1), ("Dynamics", 2)]);

    // Moving the first chapter to the end.
    let id = chapters[0]["id"].as_str().unwrap();
    let r = t.json(Method::PATCH, &format!("/api/chapters/{id}"), Some(ADMIN), json!({ "position": -1 })).await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
    let r = t
        .json(Method::PATCH, &format!("/api/chapters/{id}"), Some(ADMIN), json!({ "position": 10, "title": "Intro!" }))
        .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let course = t.get("/api/courses/phys-1").await.json();
    let titles: Vec<_> = course["chapters"].as_array().unwrap().iter().map(|c| c["title"].as_str().unwrap()).collect();
    assert_eq!(titles, ["Kinematics", "Dynamics", "Intro!"]);

    // Renaming a course's slug.
    let r = t.json(Method::PATCH, "/api/courses/bio-1", Some(ADMIN), json!({ "slug": "bio-101" })).await;
    assert_eq!(r.status, StatusCode::OK);
    assert_eq!(t.get("/api/courses/bio-1").await.status, StatusCode::NOT_FOUND);
    assert_eq!(t.get("/api/courses/bio-101").await.status, StatusCode::OK);
    let r = t.json(Method::PATCH, "/api/courses/bio-101", Some(ADMIN), json!({ "slug": "phys-1" })).await;
    assert_eq!(r.status, StatusCode::CONFLICT);
}

#[tokio::test]
async fn only_admin_can_edit_or_delete() {
    let t = TestApp::new().await;
    let (slug, chapter_id) = t.course_with_chapter("cs-50").await;
    let note = t.upload_note(&chapter_id, "Anyone can upload", &[small_jpeg()]).await;
    assert_eq!(note.status, StatusCode::CREATED);
    let note_id = note.json()["id"].as_str().unwrap().to_owned();
    let image_id = note.json()["images"][0]["id"].as_str().unwrap().to_owned();

    // Credential check endpoint used by notes-admin.
    let check = |token: Option<&'static str>| {
        let mut req = axum::http::Request::get("/api/auth/check");
        if let Some(t) = token {
            req = req.header(header::AUTHORIZATION, format!("Bearer {t}"));
        }
        req.body(axum::body::Body::empty()).unwrap()
    };
    assert_eq!(t.call(check(Some(ADMIN))).await.status, StatusCode::NO_CONTENT);
    assert_eq!(t.call(check(None)).await.status, StatusCode::UNAUTHORIZED);
    assert_eq!(t.call(check(Some("wrong"))).await.status, StatusCode::FORBIDDEN);

    let checks = [
        (Method::POST, "/api/courses".to_owned()),
        (Method::PATCH, format!("/api/courses/{slug}")),
        (Method::DELETE, format!("/api/courses/{slug}")),
        (Method::POST, format!("/api/courses/{slug}/chapters")),
        (Method::PATCH, format!("/api/chapters/{chapter_id}")),
        (Method::DELETE, format!("/api/chapters/{chapter_id}")),
        (Method::PATCH, format!("/api/notes/{note_id}")),
        (Method::DELETE, format!("/api/notes/{note_id}")),
        (Method::POST, format!("/api/notes/{note_id}/images")),
        (Method::DELETE, format!("/api/notes/{note_id}/images/{image_id}")),
    ];
    for (method, uri) in &checks {
        let r = t.json(method.clone(), uri, None, json!({})).await;
        assert_eq!(r.status, StatusCode::UNAUTHORIZED, "{method} {uri}");
        assert_eq!(r.headers[header::WWW_AUTHENTICATE], "Bearer");
        assert_eq!(r.json()["error"], "missing or malformed admin token");

        let r = t.json(method.clone(), uri, Some("wrong-token"), json!({})).await;
        assert_eq!(r.status, StatusCode::FORBIDDEN, "{method} {uri}");
    }

    // Nothing was changed by the rejected requests.
    let r = t.get(&format!("/api/notes/{note_id}")).await;
    assert_eq!(r.status, StatusCode::OK);
    assert_eq!(r.json()["title"], "Anyone can upload");
    assert_eq!(r.json()["images"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn admin_edits_moves_and_manages_images() {
    let t = TestApp::new().await;
    let (slug, chapter_a) = t.course_with_chapter("chem-1").await;
    let chapter_b = t
        .json(Method::POST, &format!("/api/courses/{slug}/chapters"), Some(ADMIN), json!({ "title": "Chapter 2" }))
        .await
        .json()["id"]
        .as_str()
        .unwrap()
        .to_owned();

    let note = t.upload_note(&chapter_a, "Draft", &[small_jpeg(), small_jpeg()]).await.json();
    let note_id = note["id"].as_str().unwrap();

    // Edit + move to another chapter.
    let r = t
        .json(
            Method::PATCH,
            &format!("/api/notes/{note_id}"),
            Some(ADMIN),
            json!({ "title": "Final", "author_name": "TA", "chapter_id": chapter_b }),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.json()["title"], "Final");
    assert_eq!(r.json()["author_name"], "TA");
    assert_eq!(r.json()["chapter_id"], chapter_b.as_str());
    assert_eq!(t.get(&format!("/api/chapters/{chapter_a}")).await.json()["total_notes"], 0);
    assert_eq!(t.get(&format!("/api/chapters/{chapter_b}")).await.json()["total_notes"], 1);
    // Image URLs survive the move (keys are per note, not per chapter).
    assert_eq!(r.json()["images"], note["images"]);

    // Clearing the author name, rejecting a bogus chapter and unknown fields.
    let r = t.json(Method::PATCH, &format!("/api/notes/{note_id}"), Some(ADMIN), json!({ "author_name": "" })).await;
    assert_eq!(r.json()["author_name"], serde_json::Value::Null);
    let r = t.json(Method::PATCH, &format!("/api/notes/{note_id}"), Some(ADMIN), json!({ "chapter_id": "nope" })).await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
    let r = t.json(Method::PATCH, &format!("/api/notes/{note_id}"), Some(ADMIN), json!({ "titel": "typo" })).await;
    assert!(r.status.is_client_error());

    // Add images: appended after the existing two.
    let form = Form::new()
        .file("images", "c.jpg", &small_jpeg())
        .file("images", "d.jpg", &small_jpeg());
    let r = t.call(form.request(&format!("/api/notes/{note_id}/images"), Some(ADMIN))).await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.text());
    let added = r.json();
    let positions: Vec<_> = added.as_array().unwrap().iter().map(|i| i["position"].as_i64().unwrap()).collect();
    assert_eq!(positions, [2, 3]);

    // Delete one image: row and both files go.
    let victim = &note["images"][0];
    let files = [t.file_for_url(victim["url"].as_str().unwrap()), t.file_for_url(victim["thumb_url"].as_str().unwrap())];
    assert!(files.iter().all(|f| f.exists()));
    let uri = format!("/api/notes/{note_id}/images/{}", victim["id"].as_str().unwrap());
    assert_eq!(t.delete(&uri, Some(ADMIN)).await.status, StatusCode::NO_CONTENT);
    assert!(files.iter().all(|f| !f.exists()));
    assert_eq!(t.delete(&uri, Some(ADMIN)).await.status, StatusCode::NOT_FOUND);

    let remaining = t.get(&format!("/api/notes/{note_id}")).await.json();
    let positions: Vec<_> = remaining["images"].as_array().unwrap().iter().map(|i| i["position"].as_i64().unwrap()).collect();
    assert_eq!(positions, [1, 2, 3]);

    // Delete the note: its whole directory goes.
    assert_eq!(t.delete(&format!("/api/notes/{note_id}"), Some(ADMIN)).await.status, StatusCode::NO_CONTENT);
    assert_eq!(t.get(&format!("/api/notes/{note_id}")).await.status, StatusCode::NOT_FOUND);
    assert!(!t.uploads().join("notes").join(note_id).exists());
}

#[tokio::test]
async fn deleting_course_or_chapter_removes_everything_below() {
    let t = TestApp::new().await;
    let (slug, chapter_id) = t.course_with_chapter("hist-1").await;
    let mut note_ids = Vec::new();
    for i in 0..3 {
        let r = t.upload_note(&chapter_id, &format!("note {i}"), &[small_jpeg()]).await;
        note_ids.push(r.json()["id"].as_str().unwrap().to_owned());
    }
    // A second course that must be untouched.
    let (_, other_chapter) = t.course_with_chapter("art-1").await;
    let survivor = t.upload_note(&other_chapter, "keep me", &[small_jpeg()]).await.json();

    assert_eq!(t.delete(&format!("/api/courses/{slug}"), Some(ADMIN)).await.status, StatusCode::NO_CONTENT);
    assert_eq!(t.get(&format!("/api/courses/{slug}")).await.status, StatusCode::NOT_FOUND);
    assert_eq!(t.get(&format!("/api/chapters/{chapter_id}")).await.status, StatusCode::NOT_FOUND);
    for id in &note_ids {
        assert_eq!(t.get(&format!("/api/notes/{id}")).await.status, StatusCode::NOT_FOUND);
        assert!(!t.uploads().join("notes").join(id).exists());
    }
    let (notes, images): (i64, i64) = sqlx::query_as(
        "SELECT (SELECT COUNT(*) FROM notes), (SELECT COUNT(*) FROM images)",
    )
    .fetch_one(&t.pool)
    .await
    .unwrap();
    assert_eq!((notes, images), (1, 1));
    assert!(t.file_for_url(survivor["images"][0]["url"].as_str().unwrap()).exists());

    // Chapter delete.
    assert_eq!(t.delete(&format!("/api/chapters/{other_chapter}"), Some(ADMIN)).await.status, StatusCode::NO_CONTENT);
    assert!(!t.file_for_url(survivor["images"][0]["url"].as_str().unwrap()).exists());
    assert_eq!(t.delete(&format!("/api/chapters/{other_chapter}"), Some(ADMIN)).await.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn upload_validation() {
    let t = TestApp::new().await;
    let (_, chapter_id) = t.course_with_chapter("val-1").await;
    let uri = format!("/api/chapters/{chapter_id}/notes");

    // Missing title.
    let r = t.call(Form::new().file("images", "a.jpg", &small_jpeg()).request(&uri, None)).await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
    assert_eq!(r.json()["error"], "`title` is required");

    // No images.
    let r = t.call(Form::new().text("title", "x").request(&uri, None)).await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);

    // An empty file part (untouched file input) is ignored, not an error by itself.
    let r = t
        .call(Form::new().text("title", "x").file("images", "", b"").file("images", "a.jpg", &small_jpeg()).request(&uri, None))
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.text());
    assert_eq!(r.json()["images"].as_array().unwrap().len(), 1);

    // Not an image (named .jpg regardless): 415, names the offending file, nothing written.
    let before = count_files(&t.uploads());
    let r = t
        .call(
            Form::new()
                .text("title", "x")
                .file("images", "ok.jpg", &small_jpeg())
                .file("images", "evil.jpg", b"<script>alert(1)</script>")
                .request(&uri, None),
        )
        .await;
    assert_eq!(r.status, StatusCode::UNSUPPORTED_MEDIA_TYPE);
    assert!(r.json()["error"].as_str().unwrap().contains("image 2 (evil.jpg)"), "{}", r.text());
    assert_eq!(count_files(&t.uploads()), before);

    // Unknown chapter.
    let r = t.upload_note("does-not-exist", "x", &[small_jpeg()]).await;
    assert_eq!(r.status, StatusCode::NOT_FOUND);
    assert_eq!(r.json()["error"], "chapter not found");

    // Too many images.
    let many = vec![small_jpeg(); 11];
    let r = t.upload_note(&chapter_id, "x", &many).await;
    assert_eq!(r.status, StatusCode::PAYLOAD_TOO_LARGE);

    // One file over 10 MB.
    let huge = vec![0u8; notes_server::routes::MAX_IMAGE_BYTES + 1];
    let r = t.call(Form::new().text("title", "x").file("images", "big.jpg", &huge).request(&uri, None)).await;
    assert_eq!(r.status, StatusCode::PAYLOAD_TOO_LARGE);

    // Unexpected field.
    let r = t
        .call(Form::new().text("title", "x").text("chapter_id", "sneaky").file("images", "a.jpg", &small_jpeg()).request(&uri, None))
        .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);

    // Over-long title.
    let r = t.upload_note(&chapter_id, &"t".repeat(201), &[small_jpeg()]).await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn course_validation() {
    let t = TestApp::new().await;
    for bad in ["", "Has Caps", "-lead", "trail-", "sp ace", "ünï", &"a".repeat(65)] {
        let r = t.json(Method::POST, "/api/courses", Some(ADMIN), json!({ "slug": bad, "name": "x" })).await;
        assert_eq!(r.status, StatusCode::BAD_REQUEST, "slug {bad:?}");
    }
    let r = t.json(Method::POST, "/api/courses", Some(ADMIN), json!({ "slug": "ok-1", "name": "  " })).await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);

    let r = t.json(Method::POST, "/api/courses", Some(ADMIN), json!({ "slug": "ok-1", "name": "One" })).await;
    assert_eq!(r.status, StatusCode::CREATED);
    let r = t.json(Method::POST, "/api/courses", Some(ADMIN), json!({ "slug": "ok-1", "name": "Two" })).await;
    assert_eq!(r.status, StatusCode::CONFLICT);

    let r = t.json(Method::POST, "/api/courses/missing/chapters", Some(ADMIN), json!({ "title": "x" })).await;
    assert_eq!(r.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn public_uploads_are_rate_limited_per_ip() {
    let t = TestApp::with(|s| {
        s.upload_burst = 2;
        s.upload_refill_secs = 3600;
    })
    .await;
    let (_, chapter_id) = t.course_with_chapter("rl-1").await;

    for _ in 0..2 {
        assert_eq!(t.upload_note(&chapter_id, "x", &[small_jpeg()]).await.status, StatusCode::CREATED);
    }
    let r = t.upload_note(&chapter_id, "x", &[small_jpeg()]).await;
    assert_eq!(r.status, StatusCode::TOO_MANY_REQUESTS);

    // Reads and admin routes are not limited.
    assert_eq!(t.get(&format!("/api/chapters/{chapter_id}")).await.status, StatusCode::OK);
    let r = t.json(Method::POST, "/api/courses", Some(ADMIN), json!({ "slug": "rl-2", "name": "x" })).await;
    assert_eq!(r.status, StatusCode::CREATED);
}

#[tokio::test]
async fn uploads_refused_when_disk_is_low() {
    let t = TestApp::with(|s| s.min_free_disk_bytes = u64::MAX).await;
    let (_, chapter_id) = t.course_with_chapter("disk-1").await;
    let r = t.upload_note(&chapter_id, "x", &[small_jpeg()]).await;
    assert_eq!(r.status, StatusCode::INSUFFICIENT_STORAGE);
}

#[tokio::test]
async fn chapter_pagination() {
    let t = TestApp::new().await;
    let (_, chapter_id) = t.course_with_chapter("page-1").await;
    for i in 0..5 {
        t.upload_note(&chapter_id, &format!("n{i}"), &[small_jpeg()]).await;
    }
    let page = t.get(&format!("/api/chapters/{chapter_id}?limit=2&offset=2")).await.json();
    assert_eq!(page["total_notes"], 5);
    let titles: Vec<_> = page["notes"].as_array().unwrap().iter().map(|n| n["title"].as_str().unwrap()).collect();
    assert_eq!(titles, ["n2", "n3"]);
    assert!(page["notes"].as_array().unwrap().iter().all(|n| n["images"].as_array().unwrap().len() == 1));
}

fn count_files(dir: &std::path::Path) -> usize {
    let Ok(entries) = std::fs::read_dir(dir) else { return 0 };
    entries
        .flatten()
        .map(|e| if e.path().is_dir() { count_files(&e.path()) } else { 1 })
        .sum()
}
