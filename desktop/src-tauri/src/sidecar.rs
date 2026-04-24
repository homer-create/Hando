use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::{mpsc, Mutex};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EncodeCommand {
    pub cmd: &'static str, // always "encode"
    pub id: String,
    pub src: String,
    pub ext: String,
    pub opts: EncodeOpts,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EncodeOpts {
    pub jpeg_quality: u32,
    pub png_quality: u32,
    pub webp_quality: u32,
    pub emit_webp: bool,
    pub emit_avif: bool,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum SidecarEvent {
    Done {
        id: String,
        tmp: String,
        #[serde(rename = "srcBytes")]
        src_bytes: u64,
        #[serde(rename = "outBytes")]
        out_bytes: u64,
        companions: Vec<Companion>,
    },
    Error {
        id: Option<String>,
        msg: String,
    },
    #[serde(rename = "skipped-no-gain")]
    SkippedNoGain {
        id: String,
        #[serde(rename = "srcBytes")]
        src_bytes: u64,
    },
    #[serde(rename = "companion-error")]
    CompanionError {
        id: String,
        ext: String,
        msg: String,
    },
    #[serde(rename = "parse-error")]
    ParseError { msg: String, line: String },
}

#[derive(Debug, Deserialize, Clone)]
pub struct Companion {
    pub ext: String,
    pub tmp: String,
    #[serde(rename = "outBytes")]
    pub out_bytes: u64,
}

pub struct Sidecar {
    child: Child,
    pub stdin: Arc<Mutex<ChildStdin>>,
    pub events: mpsc::UnboundedReceiver<SidecarEvent>,
}

impl Sidecar {
    pub async fn spawn(node_path: PathBuf, script_path: PathBuf) -> Result<Self> {
        let mut child = Command::new(&node_path)
            .arg(&script_path)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .with_context(|| format!("failed to spawn {:?}", node_path))?;

        let stdin = child.stdin.take().ok_or_else(|| anyhow!("no stdin"))?;
        let stdout = child.stdout.take().ok_or_else(|| anyhow!("no stdout"))?;

        let (tx, rx) = mpsc::unbounded_channel::<SidecarEvent>();
        tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                match serde_json::from_str::<SidecarEvent>(&line) {
                    Ok(evt) => {
                        let _ = tx.send(evt);
                    }
                    Err(err) => {
                        eprintln!("sidecar stdout parse error: {err} line: {line}");
                    }
                }
            }
        });

        Ok(Self {
            child,
            stdin: Arc::new(Mutex::new(stdin)),
            events: rx,
        })
    }

    pub async fn send(&self, cmd: &EncodeCommand) -> Result<()> {
        let mut line = serde_json::to_string(cmd)?;
        line.push('\n');
        let mut stdin = self.stdin.lock().await;
        stdin.write_all(line.as_bytes()).await?;
        stdin.flush().await?;
        Ok(())
    }

    pub async fn shutdown(mut self) {
        drop(self.stdin);
        let _ = self.child.kill().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn repo_root() -> PathBuf {
        let here = env!("CARGO_MANIFEST_DIR");
        PathBuf::from(here).join("../..").canonicalize().unwrap()
    }

    #[tokio::test]
    async fn spawns_sidecar_and_echoes_parse_error() {
        let root = repo_root();
        let script = root.join("src/sidecar.js");
        let mut sc = Sidecar::spawn(PathBuf::from("node"), script).await.unwrap();
        {
            let mut stdin = sc.stdin.lock().await;
            stdin.write_all(b"not-json\n").await.unwrap();
            stdin.flush().await.unwrap();
        }
        let evt = tokio::time::timeout(
            std::time::Duration::from_secs(3),
            sc.events.recv(),
        )
        .await
        .unwrap()
        .unwrap();
        assert!(matches!(evt, SidecarEvent::ParseError { .. }));
        sc.shutdown().await;
    }
}
