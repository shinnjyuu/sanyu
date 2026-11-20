use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, State, rejection::JsonRejection},
    http::StatusCode,
    response::Html,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};

use crate::{
    Language, MAX_TEXT_CHARS,
    local_model::{LocalModel, ModelError},
};

// A supplementary character may take two six-byte JSON surrogate escapes.
const MAX_BODY_BYTES: usize = 320 * 1024;

#[derive(Deserialize)]
struct TranslateRequest {
    from: Language,
    to: Language,
    text: String,
}

#[derive(Serialize)]
struct ErrorResponse {
    error: &'static str,
}

#[derive(Serialize)]
struct Translation {
    translation: String,
    status: &'static str,
}

type ApiError = (StatusCode, Json<ErrorResponse>);

pub(crate) fn router(model: LocalModel) -> Router {
    Router::new()
        .route("/", get(|| async { Html(include_str!("debug.html")) }))
        .route("/translate", post(translate))
        .layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
        .with_state(model)
}

async fn translate(
    State(model): State<LocalModel>,
    payload: Result<Json<TranslateRequest>, JsonRejection>,
) -> Result<Json<Translation>, ApiError> {
    let Json(request) = payload.map_err(|rejection| match rejection.status() {
        StatusCode::PAYLOAD_TOO_LARGE => error(
            StatusCode::PAYLOAD_TOO_LARGE,
            "request body exceeds 320 KiB",
        ),
        StatusCode::UNSUPPORTED_MEDIA_TYPE => error(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "Content-Type must be application/json",
        ),
        _ => error(
            StatusCode::BAD_REQUEST,
            "expected JSON with from and to set to zh-CN, en, or ja, and text set to a string",
        ),
    })?;

    if request.text.chars().take(MAX_TEXT_CHARS + 1).count() > MAX_TEXT_CHARS {
        return Err(error(
            StatusCode::PAYLOAD_TOO_LARGE,
            "text exceeds 25565 Unicode characters",
        ));
    }
    if request.text.trim().is_empty() {
        return Err(error(StatusCode::BAD_REQUEST, "text must not be blank"));
    }
    if request.from == request.to {
        return Err(error(
            StatusCode::BAD_REQUEST,
            "from and to must be different languages",
        ));
    }

    let translation = model
        .translate(request.from, request.to, request.text)
        .await
        .map_err(|cause| match cause {
            ModelError::Busy => error(StatusCode::SERVICE_UNAVAILABLE, "translation queue is full"),
            ModelError::Stopped => error(
                StatusCode::SERVICE_UNAVAILABLE,
                "translation worker is unavailable",
            ),
            ModelError::Incomplete => error(
                StatusCode::UNPROCESSABLE_ENTITY,
                "translation is incomplete",
            ),
            ModelError::Failed(cause) => {
                eprintln!("Local translation failed: {cause:#}");
                error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "local translation failed",
                )
            }
        })?;
    Ok(Json(Translation {
        translation,
        status: "translated",
    }))
}

fn error(status: StatusCode, message: &'static str) -> ApiError {
    (status, Json(ErrorResponse { error: message }))
}
