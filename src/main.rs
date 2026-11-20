mod api;
mod glossary;
mod local_model;

#[cfg(test)]
mod tests;

use std::{env, net::SocketAddr, path::PathBuf};

use anyhow::{Context, Result};
use serde::Deserialize;
use tokio::net::TcpListener;

use local_model::LocalModel;

const MAX_TEXT_CHARS: usize = 25_565;
const DEFAULT_MODEL_DIR: &str = "/usr/share/sanyu/models";
const DEFAULT_LISTEN_ADDR: &str = "0.0.0.0:8080";

#[derive(Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Language {
    #[serde(rename = "zh-CN")]
    ZhCn,
    En,
    Ja,
}

impl Language {
    fn code(self) -> &'static str {
        match self {
            Self::ZhCn => "zh",
            Self::En => "en",
            Self::Ja => "ja",
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let model_dir = env::var_os("MODEL_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_MODEL_DIR));
    let glossary_file = env::var_os("GLOSSARY_FILE").map(PathBuf::from);
    let listen_addr = env::var_os("LISTEN_ADDR").unwrap_or_else(|| DEFAULT_LISTEN_ADDR.into());
    let listen_addr: SocketAddr = listen_addr
        .to_str()
        .context("LISTEN_ADDR must be valid UTF-8")?
        .parse()
        .context("LISTEN_ADDR must be an IP address and port")?;

    let model = LocalModel::start(&model_dir, glossary_file.as_deref())?;
    let listener = TcpListener::bind(listen_addr)
        .await
        .with_context(|| format!("bind to {listen_addr}"))?;
    eprintln!("Sanyu Translate listening on {}", listener.local_addr()?);

    axum::serve(listener, api::router(model))
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("serve HTTP")
}

#[cfg(unix)]
async fn shutdown_signal() {
    use tokio::signal::unix::{SignalKind, signal};

    let mut terminate = signal(SignalKind::terminate()).expect("install SIGTERM handler");
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {}
        _ = terminate.recv() => {}
    }
}

#[cfg(not(unix))]
async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
}
