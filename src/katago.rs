//! Driver for KataGo's JSON "analysis" engine: one long-lived child process, queries written
//! as JSON lines on stdin, responses read as JSON lines from stdout and routed by `id`.

use anyhow::{anyhow, bail, Context, Result};
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::mpsc;

#[derive(Debug, Clone, serde::Serialize)]
pub struct EngineConfig {
    pub katago: PathBuf,
    pub model: PathBuf,
    pub config: PathBuf,
    /// Optional human-style network (`b18c384nbt-humanv0`) for human policy heat maps.
    pub human_model: Option<PathBuf>,
}

impl EngineConfig {
    pub fn validate(&self) -> Result<()> {
        for (what, p) in [("KataGo executable", &self.katago), ("model", &self.model), ("analysis config", &self.config)] {
            if !p.exists() {
                bail!("{} not found at {}", what, p.display());
            }
        }
        if let Some(h) = &self.human_model {
            if !h.exists() {
                bail!("human model not found at {}", h.display());
            }
        }
        Ok(())
    }
}

type Pending = Arc<Mutex<HashMap<String, mpsc::UnboundedSender<Value>>>>;

/// Output of `katago version`: (first line, backend name such as "Metal", "OpenCL", "Eigen", "CUDA").
pub fn probe_version(katago: &std::path::Path) -> (String, String) {
    let out = std::process::Command::new(katago).arg("version").output().ok();
    let text = out.and_then(|o| String::from_utf8(o.stdout).ok()).unwrap_or_default();
    let version = text.lines().next().map(|l| l.trim().to_string()).unwrap_or_else(|| "unknown".to_string());
    let backend = text
        .lines()
        .find_map(|l| l.trim().strip_prefix("Using ").and_then(|r| r.strip_suffix(" backend")).map(|b| b.to_string()))
        .unwrap_or_else(|| "unknown".to_string());
    (version, backend)
}

/// Network name from the header of a `.bin.gz` model file (e.g. `b11c768h12nbt3tflrs-fson-silu`).
pub fn probe_model_name(model: &std::path::Path) -> Option<String> {
    use std::io::Read;
    let f = std::fs::File::open(model).ok()?;
    let mut gz = flate2::read::GzDecoder::new(f);
    let mut buf = [0u8; 256];
    let n = gz.read(&mut buf).ok()?;
    let head = String::from_utf8_lossy(&buf[..n]);
    head.split_whitespace().next().map(|s| s.to_string())
}

/// KataGo processes started by other programs (KaTrain marks its own with `homeDataDir=~/.katrain`).
#[derive(Debug, Clone, serde::Serialize)]
pub struct ForeignEngine {
    pub pid: u32,
    pub owner: String,
    pub katago: Option<String>,
    pub model: Option<String>,
}

pub fn running_foreign_engines() -> Vec<ForeignEngine> {
    let out = std::process::Command::new("ps").args(["-axo", "pid=,command="]).output().ok();
    let text = out.and_then(|o| String::from_utf8(o.stdout).ok()).unwrap_or_default();
    let me = std::process::id();
    let mut v = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        let Some((pid, cmd)) = line.split_once(' ') else { continue };
        let Ok(pid) = pid.trim().parse::<u32>() else { continue };
        if pid == me || !cmd.contains("katago") || !cmd.contains(" analysis") || cmd.contains("reportAnalysisWinratesAs=BLACK") {
            continue; // not KataGo, or one of our own engines (we always pass that override)
        }
        let args: Vec<&str> = cmd.split_whitespace().collect();
        let after = |flag: &str| args.iter().position(|a| *a == flag).and_then(|i| args.get(i + 1)).map(|s| s.to_string());
        let owner = if cmd.contains(".katrain") { "KaTrain" } else { "another program" };
        let e = ForeignEngine { pid, owner: owner.to_string(), katago: args.first().map(|s| s.to_string()), model: after("-model") };
        // A shell wrapper around the engine shows the same command line; report each engine once.
        if !v.iter().any(|x: &ForeignEngine| x.katago == e.katago && x.model == e.model) {
            v.push(e);
        }
    }
    v
}

pub struct Engine {
    stdin: tokio::sync::Mutex<ChildStdin>,
    pending: Pending,
    next_id: AtomicU64,
    alive: Arc<AtomicBool>,
    pub version: String,
    /// "Metal", "OpenCL", "Eigen", "CUDA", ...
    pub backend: String,
    pub model_name: String,
    pub config: EngineConfig,
    _child: Mutex<Child>,
}

impl Engine {
    /// Spawn KataGo and wait until it answers a trivial warm-up query (which also loads the model).
    /// `cancel` aborts the startup (and kills the child) if the app quits while the model loads.
    pub async fn spawn(config: EngineConfig, cancel: tokio_util::sync::CancellationToken) -> Result<Arc<Engine>> {
        config.validate()?;

        let (version, backend) = {
            let k = config.katago.clone();
            tokio::task::spawn_blocking(move || probe_version(&k)).await.unwrap_or_else(|_| ("unknown".into(), "unknown".into()))
        };
        let model_name = {
            let m = config.model.clone();
            tokio::task::spawn_blocking(move || probe_model_name(&m)).await.ok().flatten().unwrap_or_else(|| "unknown".to_string())
        };

        let mut cmd = Command::new(&config.katago);
        cmd.arg("analysis").arg("-model").arg(&config.model).arg("-config").arg(&config.config);
        if let Some(h) = &config.human_model {
            cmd.arg("-human-model").arg(h);
        }
        // Always report from Black's perspective so the review code is independent of the cfg.
        cmd.arg("-override-config").arg("reportAnalysisWinratesAs=BLACK");
        let mut child = cmd
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .with_context(|| format!("failed to start KataGo at {}", config.katago.display()))?;

        let stdin = child.stdin.take().ok_or_else(|| anyhow!("no stdin"))?;
        let stdout = child.stdout.take().ok_or_else(|| anyhow!("no stdout"))?;
        let stderr = child.stderr.take().ok_or_else(|| anyhow!("no stderr"))?;

        let pending: Pending = Arc::new(Mutex::new(HashMap::new()));
        let alive = Arc::new(AtomicBool::new(true));

        // stdout reader: route each JSON line to whoever is waiting for that id.
        {
            let pending = pending.clone();
            let alive = alive.clone();
            tokio::spawn(async move {
                let mut lines = BufReader::new(stdout).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    if line.trim().is_empty() {
                        continue;
                    }
                    let v: Value = match serde_json::from_str(&line) {
                        Ok(v) => v,
                        Err(e) => {
                            tracing::warn!("unparseable line from KataGo: {} ({})", line, e);
                            continue;
                        }
                    };
                    let id = v.get("id").and_then(|i| i.as_str()).map(|s| s.to_string());
                    match id {
                        Some(id) => {
                            let tx = pending.lock().unwrap().get(&id).cloned();
                            match tx {
                                Some(tx) => {
                                    let _ = tx.send(v);
                                }
                                None => tracing::debug!("response for unknown/finished query {}", id),
                            }
                        }
                        None => tracing::warn!("KataGo message without id: {}", line),
                    }
                }
                tracing::error!("KataGo stdout closed; engine is no longer available");
                alive.store(false, Ordering::SeqCst);
                // Dropping the senders wakes every waiting query with `None`.
                pending.lock().unwrap().clear();
            });
        }

        // stderr reader: KataGo logs startup info and warnings here.
        tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                tracing::info!(target: "katago", "{}", line);
            }
        });

        let engine = Arc::new(Engine {
            stdin: tokio::sync::Mutex::new(stdin),
            pending,
            next_id: AtomicU64::new(1),
            alive,
            version,
            backend,
            model_name,
            config,
            _child: Mutex::new(child),
        });

        // Warm-up: forces model load and proves the pipeline works end to end.
        let warmup = serde_json::json!({
            "moves": [],
            "rules": "chinese",
            "komi": 7.5,
            "boardXSize": 19,
            "boardYSize": 19,
            "analyzeTurns": [0],
            "maxVisits": 1,
            "includePolicy": false,
        });
        let (id, mut rx) = engine.query(warmup).await?;
        let resp = tokio::select! {
            r = rx.recv() => r,
            _ = cancel.cancelled() => {
                engine.shutdown();
                bail!("startup cancelled");
            }
        };
        engine.unregister(&id);
        match resp {
            Some(v) if v.get("error").is_some() => bail!("KataGo warm-up query failed: {}", v),
            Some(_) => {}
            None => bail!("KataGo exited during startup; run it by hand to see the error"),
        }
        tracing::info!("KataGo ready ({}, {} backend, network {})", engine.version, engine.backend, engine.model_name);
        Ok(engine)
    }

    pub fn is_alive(&self) -> bool {
        self.alive.load(Ordering::SeqCst)
    }

    /// Kill the KataGo process (used when the app quits).
    pub fn shutdown(&self) {
        self.alive.store(false, Ordering::SeqCst);
        if let Ok(mut child) = self._child.lock() {
            let _ = child.start_kill();
        }
    }

    /// Send a query. The caller receives every message KataGo emits for that id.
    pub async fn query(&self, mut q: Value) -> Result<(String, mpsc::UnboundedReceiver<Value>)> {
        if !self.is_alive() {
            bail!("KataGo process is not running");
        }
        let id = format!("q{}", self.next_id.fetch_add(1, Ordering::SeqCst));
        q["id"] = Value::String(id.clone());
        let (tx, rx) = mpsc::unbounded_channel();
        self.pending.lock().unwrap().insert(id.clone(), tx);
        let mut line = serde_json::to_string(&q)?;
        line.push('\n');
        let mut stdin = self.stdin.lock().await;
        if let Err(e) = stdin.write_all(line.as_bytes()).await {
            self.unregister(&id);
            bail!("failed to write to KataGo: {}", e);
        }
        stdin.flush().await?;
        Ok((id, rx))
    }

    /// Ask KataGo to stop working on a query.
    pub async fn terminate(&self, id: &str) -> Result<()> {
        let q = serde_json::json!({
            "id": format!("{}-terminate", id),
            "action": "terminate",
            "terminateId": id,
        });
        let mut line = serde_json::to_string(&q)?;
        line.push('\n');
        let mut stdin = self.stdin.lock().await;
        stdin.write_all(line.as_bytes()).await?;
        stdin.flush().await?;
        Ok(())
    }

    pub fn unregister(&self, id: &str) {
        self.pending.lock().unwrap().remove(id);
    }
}
