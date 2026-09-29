//! Makes sure `frontend/dist` exists so the server compiles (and its tests run)
//! without Node installed. The real site comes from `npm run build` in
//! `frontend/`, which replaces the placeholder.

use std::path::Path;

fn main() {
    let dist = Path::new("frontend/dist");
    if !dist.join("index.html").exists() {
        std::fs::create_dir_all(dist).expect("create frontend/dist");
        std::fs::write(
            dist.join("index.html"),
            "<!doctype html><meta charset=utf-8><title>notes</title>\
             <p>The frontend has not been built yet: run <code>npm ci && npm run build</code> in <code>frontend/</code>.</p>",
        )
        .expect("write placeholder index.html");
    }
    // Release builds embed dist/; rebuild when it changes.
    println!("cargo:rerun-if-changed=frontend/dist");
    // `sqlx::migrate!` embeds migrations/; rebuild when one is added.
    println!("cargo:rerun-if-changed=migrations");
    println!("cargo:rerun-if-changed=build.rs");
}
