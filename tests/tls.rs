//! Runs the real HTTPS listener (rustls + ring) and the HTTP→HTTPS redirect.

mod common;

use std::net::SocketAddr;

use axum_server::Handle;
use common::{ADMIN, TestApp, jpeg};
use notes_server::tls;

#[tokio::test]
async fn https_end_to_end_and_http_redirect() {
    tls::install_crypto_provider();

    // Self-signed certificate written to disk, loaded the same way as in production.
    let certified = rcgen::generate_simple_self_signed(vec!["localhost".to_owned()]).unwrap();
    let cert_pem = certified.cert.pem();
    let certs = tempfile::tempdir().unwrap();
    let (cert_path, key_path) = (certs.path().join("cert.pem"), certs.path().join("key.pem"));
    std::fs::write(&cert_path, &cert_pem).unwrap();
    std::fs::write(&key_path, certified.signing_key.serialize_pem()).unwrap();
    let tls_config = tls::load(&cert_path, &key_path).await.unwrap();
    // Reload works with the same files (what the periodic reloader does).
    tls_config.reload_from_pem_file(&cert_path, &key_path).await.unwrap();

    let t = TestApp::new().await;
    let handle = Handle::new();
    tokio::spawn(notes_server::serve_https(
        t.app.clone(),
        SocketAddr::from(([127, 0, 0, 1], 0)),
        tls_config,
        handle.clone(),
    ));
    let port = handle.listening().await.expect("server started").port();
    let base = format!("https://localhost:{port}");

    let client = reqwest::Client::builder()
        .tls_certs_only([reqwest::Certificate::from_pem(cert_pem.as_bytes()).unwrap()])
        .build()
        .unwrap();

    let res = client.get(format!("{base}/health")).send().await.unwrap();
    assert_eq!(res.status(), 200);
    assert!(res.headers().contains_key("strict-transport-security"));
    assert_eq!(res.text().await.unwrap(), "ok");

    // Admin setup + public multipart upload over TLS.
    let res = client
        .post(format!("{base}/api/courses"))
        .bearer_auth(ADMIN)
        .json(&serde_json::json!({ "slug": "tls-101", "name": "TLS" }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 201);
    let chapter: serde_json::Value = client
        .post(format!("{base}/api/courses/tls-101/chapters"))
        .bearer_auth(ADMIN)
        .json(&serde_json::json!({ "title": "Handshakes" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    let form = reqwest::multipart::Form::new().text("title", "Over HTTPS").part(
        "images",
        reqwest::multipart::Part::bytes(jpeg(2400, 1800)).file_name("photo.jpg"),
    );
    let res = client
        .post(format!("{base}/api/chapters/{}/notes", chapter["id"].as_str().unwrap()))
        .multipart(form)
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 201);
    let note: serde_json::Value = res.json().await.unwrap();
    assert_eq!(note["images"][0]["width"], 1600);
    assert_eq!(note["images"][0]["height"], 1200);

    let url = note["images"][0]["url"].as_str().unwrap();
    let path = &url[url.find("/files/").unwrap()..];
    let res = client.get(format!("{base}{path}")).send().await.unwrap();
    assert_eq!(res.status(), 200);
    assert_eq!(res.headers()["content-type"], "image/jpeg");

    // Plain HTTP is redirected to the configured HTTPS origin, except ACME
    // challenges, which are served from the webroot for certbot renewals.
    let webroot = tempfile::tempdir().unwrap();
    let challenge_dir = webroot.path().join(".well-known").join("acme-challenge");
    std::fs::create_dir_all(&challenge_dir).unwrap();
    std::fs::write(challenge_dir.join("tok3n"), "tok3n.key-auth").unwrap();

    let redirect_handle = Handle::new();
    tokio::spawn({
        let redirect_handle = redirect_handle.clone();
        let webroot = webroot.path().to_owned();
        async move {
            tls::serve_redirect(
                SocketAddr::from(([127, 0, 0, 1], 0)),
                "https://notes.example.com",
                Some(&webroot),
                redirect_handle,
            )
            .await
        }
    });
    let http_port = redirect_handle.listening().await.unwrap().port();
    let no_follow = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    let res = no_follow
        .get(format!("http://127.0.0.1:{http_port}/api/courses?page=2"))
        .header("host", "evil.example")
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 308);
    assert_eq!(res.headers()["location"], "https://notes.example.com/api/courses?page=2");

    let res = no_follow
        .get(format!("http://127.0.0.1:{http_port}/.well-known/acme-challenge/tok3n"))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    assert_eq!(res.text().await.unwrap(), "tok3n.key-auth");

    handle.shutdown();
    redirect_handle.shutdown();
}
