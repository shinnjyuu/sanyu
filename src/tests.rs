use std::{fs, path::Path};

use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use serde_json::{Value, json};
use tempfile::TempDir;
use tower::ServiceExt;

use crate::{Language, MAX_TEXT_CHARS, api, glossary::Glossary, local_model::LocalModel};

#[tokio::test]
async fn serves_the_debug_form_at_the_root() {
    let directory = fixture_models();
    let response = router(&directory, None)
        .oneshot(Request::get("/").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers()["content-type"],
        "text/html; charset=utf-8"
    );
    let body = to_bytes(response.into_body(), 64 * 1024).await.unwrap();
    let html = std::str::from_utf8(&body).unwrap();
    assert!(html.contains("id=\"translate-form\""));
}

#[tokio::test]
async fn validates_languages_text_and_body_limits() {
    let directory = fixture_models();
    let app = router(&directory, None);
    for request in [
        json!({"from": "es", "to": "ja", "text": "hola"}),
        json!({"from": "en", "to": "en", "text": "hello"}),
        json!({"from": "en", "to": "ja", "text": " \n\t"}),
        json!({"from": "en", "to": "ja", "text": 3}),
    ] {
        assert_eq!(
            post(&app, "application/json", request.to_string()).await.0,
            StatusCode::BAD_REQUEST
        );
    }
    assert_eq!(
        post(&app, "application/json", "{".into()).await.0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        post(&app, "text/plain", "hello".into()).await.0,
        StatusCode::UNSUPPORTED_MEDIA_TYPE
    );

    let at_limit = format!(
        r#"{{"from":"en","to":"en","text":"{}"}}"#,
        r"\uD842\uDFB7".repeat(MAX_TEXT_CHARS)
    );
    let (status, body) = post(&app, "application/json", at_limit).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"], "from and to must be different languages");

    let too_many_chars =
        json!({"from": "zh-CN", "to": "en", "text": "字".repeat(MAX_TEXT_CHARS + 1)});
    let (status, body) = post(&app, "application/json", too_many_chars.to_string()).await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(body["error"], "text exceeds 25565 Unicode characters");

    let (status, body) = post(&app, "application/json", " ".repeat(320 * 1024 + 1)).await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(body["error"], "request body exceeds 320 KiB");
}

#[tokio::test]
async fn glossary_prefers_longer_terms_and_preserves_line_endings() {
    let directory = fixture_models();
    let glossary = directory.path().join("glossary.json");
    let entries = json!([
        {"from":"en","to":"ja","text":"new york","translation":"ニューヨーク"},
        {"from":"en","to":"ja","text":"new york city","translation":"ニューヨーク市"},
        {"from":"zh-CN","to":"en","text":"圖書館","translation":"library"},
        {"from":"zh-CN","to":"en","text":"書","translation":"book"},
        {"from":"ja","to":"zh-CN","text":"図書館","translation":"圖書館"},
    ]);
    fs::write(&glossary, entries.to_string()).unwrap();
    let app = router(&directory, Some(&glossary));
    for (from, to, text, expected) in [
        (
            "en",
            "ja",
            "New York City\r\nNew York. 42",
            "ニューヨーク市\r\nニューヨーク. 42",
        ),
        ("zh-CN", "en", "图书馆书", "library book"),
        ("ja", "zh-CN", "図書館", "图书馆"),
    ] {
        let request = json!({"from": from, "to": to, "text": text});
        let (status, body) = post(&app, "application/json", request.to_string()).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            body,
            json!({"translation": expected, "status": "translated"})
        );
    }
}

#[test]
fn glossary_matches_english_terms_at_word_boundaries() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("glossary.json");
    fs::write(
        &path,
        r#"[{"from":"en","to":"ja","text":"can","translation":"できる"}]"#,
    )
    .unwrap();
    let glossary = Glossary::load(&path).unwrap();
    let terms = glossary.get(Language::En, Language::Ja).unwrap();
    assert_eq!(terms.longest_match("scant", 1, true), None);
    assert_eq!(terms.longest_match("can’t", 0, true), None);
    assert_eq!(terms.longest_match("'can'", 1, true), Some((4, "できる")));
}

#[test]
fn identifies_missing_assets_and_invalid_glossary_at_startup() {
    let directory = fixture_models();
    let glossary_path = directory.path().join("glossary.json");
    fs::write(&glossary_path, r#"{"hello":"こんにちは"}"#).unwrap();
    let Err(error) = LocalModel::start(directory.path(), Some(&glossary_path)) else {
        panic!("expected invalid glossary to fail startup");
    };
    assert!(format!("{error:#}").contains("parse glossary JSON array"));

    let missing_asset = directory.path().join("zh-en/model.bin");
    fs::remove_file(&missing_asset).unwrap();
    let Err(error) = LocalModel::start(directory.path(), None) else {
        panic!("expected missing model to fail startup");
    };
    assert!(format!("{error:#}").contains(&missing_asset.display().to_string()));
}

fn fixture_models() -> TempDir {
    let directory = tempfile::tempdir().unwrap();
    // These tests exercise validation and fully overridden requests without running inference.
    for pair in ["en-ja", "en-zh", "ja-en", "zh-en"] {
        let pair = directory.path().join(pair);
        fs::create_dir(&pair).unwrap();
        fs::write(pair.join("model.bin"), "fixture").unwrap();
        fs::write(pair.join("vocab.spm"), "fixture").unwrap();
    }
    directory
}

fn router(directory: &TempDir, glossary: Option<&Path>) -> Router {
    api::router(LocalModel::start(directory.path(), glossary).unwrap())
}

async fn post(app: &Router, content_type: &str, body: String) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(
            Request::post("/translate")
                .header("content-type", content_type)
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 64 * 1024).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap())
}
