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

#[tokio::test]
async fn serves_the_frontend_and_spa_routes() {
    let t = TestApp::new().await;

    let r = t.get("/").await;
    assert_eq!(r.status, StatusCode::OK);
    assert!(r.headers[header::CONTENT_TYPE].to_str().unwrap().starts_with("text/html"));
    assert_eq!(r.headers[header::CACHE_CONTROL], "no-cache");
    assert!(r.headers.contains_key(header::CONTENT_SECURITY_POLICY));
    let etag = r.headers[header::ETAG].clone();

    // Client-side routes get the same shell, so deep links work.
    let deep = t.get("/course/math-101/chapter/abc").await;
    assert_eq!(deep.status, StatusCode::OK);
    assert_eq!(deep.body, r.body);

    // Revalidation.
    let req = axum::http::Request::get("/")
        .header(header::IF_NONE_MATCH, etag)
        .body(axum::body::Body::empty())
        .unwrap();
    assert_eq!(t.call(req).await.status, StatusCode::NOT_MODIFIED);

    // Unknown API routes and assets are real 404s, never the HTML shell.
    let r = t.get("/api/nope").await;
    assert_eq!(r.status, StatusCode::NOT_FOUND);
    assert_eq!(r.json()["error"], "endpoint not found");
    assert_eq!(t.get("/assets/missing-abc123.js").await.status, StatusCode::NOT_FOUND);
    let r = t.json(Method::POST, "/somewhere", None, json!({})).await;
    assert_eq!(r.status, StatusCode::METHOD_NOT_ALLOWED);
}

#[tokio::test]
async fn stats_and_listing_summaries() {
    let t = TestApp::new().await;
    let (slug, chapter_id) = t.course_with_chapter("sum-1").await;
    t.upload_note(&chapter_id, "a", &[small_jpeg(), small_jpeg()]).await;
    let newest = t.upload_note(&chapter_id, "b", &[small_jpeg()]).await.json();

    let stats = t.get("/api/stats").await.json();
    assert_eq!((stats["courses"].as_i64(), stats["chapters"].as_i64()), (Some(1), Some(1)));
    assert_eq!((stats["notes"].as_i64(), stats["images"].as_i64()), (Some(2), Some(3)));
    assert_eq!(stats["requests"].as_array().unwrap().len(), 13);
    assert_eq!(stats["uploads"].as_array().unwrap().len(), 13);
    assert_eq!(stats["status"], "starting"); // the reporter isn't running in tests

    let courses = t.get("/api/courses").await.json();
    let c = &courses[0];
    assert_eq!((c["chapter_count"].as_i64(), c["note_count"].as_i64(), c["image_count"].as_i64()), (Some(1), Some(2), Some(3)));
    assert_eq!(c["cover_thumb_url"], newest["images"][0]["thumb_url"]);

    let detail = t.get(&format!("/api/courses/{slug}")).await.json();
    let ch = &detail["chapters"][0];
    assert_eq!((ch["note_count"].as_i64(), ch["image_count"].as_i64()), (Some(2), Some(3)));
    assert_eq!(ch["cover_thumb_url"], newest["images"][0]["thumb_url"]);
    assert_eq!(ch["title"], "Chapter 1"); // flattened chapter fields are still there
}

#[tokio::test]
async fn chapter_notes_newest_first() {
    let t = TestApp::new().await;
    let (_, chapter_id) = t.course_with_chapter("order-1").await;
    for i in 0..3 {
        t.upload_note(&chapter_id, &format!("n{i}"), &[small_jpeg()]).await;
    }
    let titles = |v: serde_json::Value| -> Vec<String> {
        v["notes"].as_array().unwrap().iter().map(|n| n["title"].as_str().unwrap().to_owned()).collect()
    };
    assert_eq!(titles(t.get(&format!("/api/chapters/{chapter_id}?order=desc&limit=2")).await.json()), ["n2", "n1"]);
    assert_eq!(titles(t.get(&format!("/api/chapters/{chapter_id}?order=desc&limit=2&offset=2")).await.json()), ["n0"]);
    assert_eq!(titles(t.get(&format!("/api/chapters/{chapter_id}?order=asc")).await.json()), ["n0", "n1", "n2"]);
    assert_eq!(t.get(&format!("/api/chapters/{chapter_id}?order=sideways")).await.status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn course_cover_photo() {
    let t = TestApp::new().await;
    let (slug, chapter_id) = t.course_with_chapter("cov-1").await;
    let note = t.upload_note(&chapter_id, "a", &[small_jpeg()]).await.json();
    let cover_uri = format!("/api/courses/{slug}/cover");
    let put = |token: Option<&str>, form: Form| form.request_with(Method::PUT, &cover_uri, token);
    let big = || Form::new().file("image", "cover.jpg", &jpeg(3000, 2000));

    // Without a custom cover, the newest photo is used.
    let c = &t.get("/api/courses").await.json()[0];
    assert_eq!(c["custom_cover"], false);
    assert_eq!(c["cover_thumb_url"], note["images"][0]["thumb_url"]);
    assert_eq!(c["cover_url"], note["images"][0]["url"]);

    // Admin only.
    assert_eq!(t.call(put(None, big())).await.status, StatusCode::UNAUTHORIZED);
    assert_eq!(t.call(put(Some("wrong-token-wrong-token-wrong-token"), big())).await.status, StatusCode::FORBIDDEN);
    assert_eq!(t.delete(&cover_uri, None).await.status, StatusCode::UNAUTHORIZED);

    // Set: resized like note photos, and it wins over the automatic cover.
    let r = t.call(put(Some(ADMIN), big())).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let set = r.json();
    assert_eq!(set["custom_cover"], true);
    assert_eq!(set["slug"], slug.as_str());
    let main = set["cover_url"].as_str().unwrap().to_owned();
    let thumb = set["cover_thumb_url"].as_str().unwrap().to_owned();
    assert!(main.starts_with(&format!("{BASE_URL}/files/covers/")), "{main}");
    let size = |url: &str| {
        let img = image::load_from_memory(&std::fs::read(t.file_for_url(url)).unwrap()).unwrap();
        (img.width(), img.height())
    };
    assert_eq!(size(&main), (1600, 1067));
    assert_eq!(size(&thumb), (300, 200));
    assert_eq!(t.get(main.strip_prefix(BASE_URL).unwrap()).await.status, StatusCode::OK);
    let c = &t.get("/api/courses").await.json()[0];
    assert_eq!(c["cover_thumb_url"], thumb.as_str());
    let d = t.get(&format!("/api/courses/{slug}")).await.json();
    assert_eq!((d["cover_url"].as_str(), d["custom_cover"].as_bool()), (Some(main.as_str()), Some(true)));
    assert_eq!(d["chapters"][0]["id"], chapter_id.as_str());

    // Newer notes don't replace a custom cover.
    t.upload_note(&chapter_id, "b", &[small_jpeg()]).await;
    assert_eq!(t.get("/api/courses").await.json()[0]["cover_thumb_url"], thumb.as_str());

    // Replacing deletes the old files; the new one gets a new URL (cached forever).
    let r = t.call(put(Some(ADMIN), Form::new().file("image", "b.jpg", &small_jpeg()))).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let new_main = r.json()["cover_url"].as_str().unwrap().to_owned();
    assert_ne!(new_main, main);
    assert!(t.file_for_url(&new_main).exists());
    assert!(!t.file_for_url(&main).exists() && !t.file_for_url(&thumb).exists());

    // Bad requests leave the cover alone.
    let r = t.call(put(Some(ADMIN), Form::new().file("image", "x.txt", b"not an image"))).await;
    assert_eq!(r.status, StatusCode::UNSUPPORTED_MEDIA_TYPE);
    let two = Form::new().file("image", "a.jpg", &small_jpeg()).file("image", "b.jpg", &small_jpeg());
    assert_eq!(t.call(put(Some(ADMIN), two)).await.status, StatusCode::BAD_REQUEST);
    assert_eq!(t.call(put(Some(ADMIN), Form::new())).await.status, StatusCode::BAD_REQUEST);
    let missing = Form::new()
        .file("image", "a.jpg", &small_jpeg())
        .request_with(Method::PUT, "/api/courses/nope/cover", Some(ADMIN));
    assert_eq!(t.call(missing).await.status, StatusCode::NOT_FOUND);
    assert_eq!(t.get("/api/courses").await.json()[0]["cover_url"], new_main.as_str());

    // Remove: back to automatic, files gone.
    assert_eq!(t.delete(&cover_uri, Some(ADMIN)).await.status, StatusCode::NO_CONTENT);
    let c = &t.get("/api/courses").await.json()[0];
    assert_eq!(c["custom_cover"], false);
    assert!(c["cover_url"].as_str().unwrap().contains("/files/notes/"));
    assert!(!t.file_for_url(&new_main).exists());
    assert_eq!(t.delete(&cover_uri, Some(ADMIN)).await.status, StatusCode::NO_CONTENT, "removing twice is fine");

    // Deleting the course removes its cover files.
    assert_eq!(t.call(put(Some(ADMIN), big())).await.status, StatusCode::OK);
    assert_eq!(count_files(&t.uploads().join("covers")), 2);
    assert_eq!(t.delete(&format!("/api/courses/{slug}"), Some(ADMIN)).await.status, StatusCode::NO_CONTENT);
    assert_eq!(count_files(&t.uploads().join("covers")), 0);
}

#[tokio::test]
async fn rate_limit_sees_real_clients_through_the_tunnel() {
    use axum::extract::ConnectInfo;
    use std::net::SocketAddr;

    let t = TestApp::with(|s| {
        s.upload_burst = 1;
        s.upload_refill_secs = 3600;
    })
    .await;
    let (_, chapter_id) = t.course_with_chapter("cf-1").await;
    let upload = |peer: [u8; 4], cf_ip: Option<&str>| {
        let mut req = Form::new()
            .text("title", "x")
            .file("images", "a.jpg", &small_jpeg())
            .request(&format!("/api/chapters/{chapter_id}/notes"), None);
        req.extensions_mut().insert(ConnectInfo(SocketAddr::from((peer, 5000))));
        if let Some(ip) = cf_ip {
            req.headers_mut().insert("cf-connecting-ip", ip.parse().unwrap());
        }
        req
    };

    // Through cloudflared (loopback), each visitor gets their own limit.
    assert_eq!(t.call(upload([127, 0, 0, 1], Some("203.0.113.7"))).await.status, StatusCode::CREATED);
    assert_eq!(t.call(upload([127, 0, 0, 1], Some("203.0.113.8"))).await.status, StatusCode::CREATED);
    assert_eq!(t.call(upload([127, 0, 0, 1], Some("203.0.113.7"))).await.status, StatusCode::TOO_MANY_REQUESTS);

    // A client on the LAN can't dodge its limit by sending the header itself.
    assert_eq!(t.call(upload([192, 168, 1, 50], None)).await.status, StatusCode::CREATED);
    let r = t.call(upload([192, 168, 1, 50], Some("198.51.100.1"))).await;
    assert_eq!(r.status, StatusCode::TOO_MANY_REQUESTS);
}
