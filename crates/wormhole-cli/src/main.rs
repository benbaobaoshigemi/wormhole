use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use reqwest::Client;
use serde_json::json;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "wormhole")]
struct Args {
    #[arg(long, default_value = "http://127.0.0.1:53317")]
    api: String,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    State,
    Connect,
    Send { paths: Vec<PathBuf> },
    Cancel { task_id: String },
    Retry,
    Tasks,
    History,
    ClearHistory,
    ClipboardText,
    ClipboardImage,
    ClipboardEnable,
    ClipboardDisable,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let client = Client::new();
    let url = |path: &str| format!("{}{}", args.api.trim_end_matches('/'), path);
    let response = match args.command {
        Command::State => client.get(url("/api/state")).send().await?,
        Command::Connect => client.post(url("/api/connect")).send().await?,
        Command::Send { paths } => {
            let paths = paths
                .into_iter()
                .map(|p| p.to_string_lossy().to_string())
                .collect::<Vec<_>>();
            client
                .post(url("/api/transfer/send"))
                .json(&json!({ "paths": paths }))
                .send()
                .await?
        }
        Command::Cancel { task_id } => {
            client
                .post(url("/api/transfer/cancel"))
                .json(&json!({ "task_id": task_id }))
                .send()
                .await?
        }
        Command::Retry => client.post(url("/api/transfer/retry")).send().await?,
        Command::Tasks => client.get(url("/api/transfer/tasks")).send().await?,
        Command::History => client.get(url("/api/transfer/history")).send().await?,
        Command::ClearHistory => {
            client
                .post(url("/api/transfer/history/clear"))
                .send()
                .await?
        }
        Command::ClipboardText => {
            client
                .post(url("/api/clipboard/system/read-send-text"))
                .send()
                .await?
        }
        Command::ClipboardImage => {
            client
                .post(url("/api/clipboard/system/read-send-image"))
                .send()
                .await?
        }
        Command::ClipboardEnable => client.post(url("/api/clipboard/enable")).send().await?,
        Command::ClipboardDisable => client.post(url("/api/clipboard/disable")).send().await?,
    };
    let status = response.status();
    let text = response.text().await.context("read response body")?;
    println!("{text}");
    if !status.is_success() {
        std::process::exit(1);
    }
    Ok(())
}
