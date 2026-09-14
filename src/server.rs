//! Local web UI: upload an SGF, watch progress, download the report.

use crate::analysis::{analyze_game, AnalysisOptions, Candidate, GameAnalysis, TurnEval};
use crate::board::Board;
use crate::sgf::{Color, GameRecord};
use crate::katago::Engine;
use crate::report::render_markdown;
use crate::sgf::parse_game;
use anyhow::Result;
use axum::{
    body::Body,
    extract::{DefaultBodyLimit, Multipart, Path, State},
    http::{header, StatusCode},
    response::{Html, IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio_util::sync::CancellationToken;

const INDEX_HTML: &str = include_str!("../static/index.html");

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum JobState {
    Queued,
    Running { done: usize, total: usize },
    Done { report_path: String, json_path: String, summary: JobSummary },
    Failed { error: String },
    Cancelled,
}

#[derive(Debug, Clone, Serialize)]
pub struct JobSummary {
    pub moves: usize,
    pub black: String,
    pub white: String,
    pub result: Option<String>,
    pub final_winrate_black: f64,
    pub final_score_lead: f64,
    pub elapsed_seconds: f64,
    pub black_mean_loss: f64,
    pub white_mean_loss: f64,
}

/// What the live viewer needs for one analysed position.
#[derive(Debug, Clone, Serialize)]
pub struct LiveTurn {
    pub turn: usize,
    pub to_move: Color,
    pub winrate: f64,
    pub score_lead: f64,
    pub visits: u64,
    pub candidates: Vec<Candidate>,
    pub ownership: Option<Vec<f32>>,
    pub policy: Option<Vec<f32>>,
    pub human_policy: Option<Vec<f32>>,
}

impl From<&TurnEval> for LiveTurn {
    fn from(t: &TurnEval) -> Self {
        LiveTurn {
            turn: t.turn,
            to_move: t.to_move,
            winrate: t.winrate,
            score_lead: t.score_lead,
            visits: t.visits,
            candidates: t.candidates.iter().take(8).cloned().collect(),
            ownership: t.ownership.clone(),
            policy: t.policy.clone(),
            human_policy: t.human_policy.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Job {
    pub id: u64,
    pub file_name: String,
    pub created_at: String,
    pub max_visits: Option<u64>,
    pub human_profile: Option<String>,
    pub state: JobState,
    #[serde(skip)]
    pub cancel: CancellationToken,
    #[serde(skip)]
    pub markdown: Option<Arc<String>>,
    #[serde(skip)]
    pub json: Option<Arc<String>>,
    #[serde(skip)]
    pub game: Arc<GameRecord>,
    #[serde(skip)]
    pub live: Vec<Option<LiveTurn>>,
    /// Number of positions analysed so far (also available while running).
    pub turns_done: usize,
    pub turns_total: usize,
}

pub enum EngineState {
    Starting,
    Ready(Arc<Engine>),
    Failed(String),
}

pub struct AppState {
    pub engine: std::sync::RwLock<EngineState>,
    pub engine_config: std::sync::RwLock<crate::katago::EngineConfig>,
    /// Where each engine file came from (flag, settings file, KaTrain, built-in).
    pub engine_source: std::sync::RwLock<String>,
    /// Command-line / environment overrides, re-applied on every engine (re)start.
    pub cli_paths: crate::EnginePaths,
    pub settings_path: PathBuf,
    pub katrain_installed: bool,
    /// True when the UI is shown in the native window rather than a browser.
    pub windowed: bool,
    pub out_dir: PathBuf,
    pub jobs: Mutex<BTreeMap<u64, Job>>,
    pub next_job: Mutex<u64>,
    /// Cancelled when the user asks the app to quit.
    pub shutdown: CancellationToken,
    /// Jobs run one at a time so a batch of uploads finishes in order; others wait as Queued.
    pub job_slots: Arc<tokio::sync::Semaphore>,
}

type Shared = Arc<AppState>;

impl AppState {
    pub fn engine(&self) -> Option<Arc<Engine>> {
        match &*self.engine.read().unwrap() {
            EngineState::Ready(e) => Some(e.clone()),
            _ => None,
        }
    }
}

pub fn router(state: Shared) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/api/status", get(status))
        .route("/api/quit", post(quit))
        .route("/api/settings", get(get_settings).post(save_settings))
        .route("/api/engine/restart", post(restart_engine))
        .route("/api/open-out-dir", post(open_out_dir))
        .route("/api/jobs/:id/open", post(open_report))
        .route("/api/jobs/:id/reveal", post(reveal_report))
        .route("/api/analyze", post(analyze))
        .route("/api/jobs", get(list_jobs))
        .route("/api/jobs/:id", get(get_job))
        .route("/api/jobs/:id/cancel", post(cancel_job))
        .route("/api/jobs/cancel-all", post(cancel_all))
        .route("/api/jobs/:id/report.md", get(report_md))
        .route("/api/jobs/:id/report.json", get(report_json))
        .route("/api/jobs/:id/live", get(live_summary))
        .route("/api/jobs/:id/turn/:turn", get(live_turn))
        .route("/report/:id", get(report_page))
        .route("/api/files", get(list_files))
        .route("/api/files/gc", post(gc_files))
        .route("/api/files/:name", axum::routing::delete(delete_file))
        .route("/api/files/:name/load", post(load_file))
        .layer(DefaultBodyLimit::max(20 * 1024 * 1024))
        .with_state(state)
}

async fn index() -> Html<&'static str> {
    Html(INDEX_HTML)
}

async fn status(State(s): State<Shared>) -> Json<serde_json::Value> {
    let (state, alive, version, error, backend, model_name) = match &*s.engine.read().unwrap() {
        EngineState::Starting => ("starting", false, String::new(), String::new(), String::new(), String::new()),
        EngineState::Ready(e) => ("ready", e.is_alive(), e.version.clone(), String::new(), e.backend.clone(), e.model_name.clone()),
        EngineState::Failed(msg) => ("failed", false, String::new(), msg.clone(), String::new(), String::new()),
    };
    let foreign = crate::katago::running_foreign_engines();
    let cfg_model = s.engine_config.read().unwrap().model.display().to_string();
    let foreign_same_model = foreign.iter().any(|f| f.model.as_deref() == Some(cfg_model.as_str()));
    let cfg = s.engine_config.read().unwrap().clone();
    Json(serde_json::json!({
        "engine_state": state,
        "engine_alive": alive,
        "engine_version": version,
        "engine_error": error,
        "backend": backend,
        "model_name": model_name,
        "foreign_engines": foreign,
        "foreign_same_model": foreign_same_model,
        "queued_jobs": s.jobs.lock().unwrap().values().filter(|j| matches!(j.state, JobState::Queued)).count(),
        "windowed": s.windowed,
        "katago": cfg.katago,
        "model": cfg.model,
        "config": cfg.config,
        "out_dir": s.out_dir,
        "human_model": cfg.human_model.is_some(),
        "human_model_path": cfg.human_model,
        "engine_source": *s.engine_source.read().unwrap(),
        "katrain_installed": s.katrain_installed,
        "settings_path": s.settings_path,
        "human_profiles": HUMAN_PROFILES,
        "running_jobs": s.jobs.lock().unwrap().values().filter(|j| matches!(j.state, JobState::Running { .. } | JobState::Queued)).count(),
    }))
}

async fn get_settings(State(s): State<Shared>) -> Json<serde_json::Value> {
    let saved = crate::load_settings();
    let cfg = s.engine_config.read().unwrap().clone();
    Json(serde_json::json!({
        "path": s.settings_path,
        "saved": saved,
        "active": {
            "katago": cfg.katago,
            "model": cfg.model,
            "config": cfg.config,
            "human_model": cfg.human_model,
        },
        "source": *s.engine_source.read().unwrap(),
    }))
}

/// Save engine paths. Empty strings clear a setting (back to automatic). Takes effect on relaunch.
async fn save_settings(State(s): State<Shared>, Json(body): Json<serde_json::Value>) -> Response {
    let mut saved = crate::load_settings();
    let mut problems = Vec::new();
    for (key, slot) in [
        ("katago", &mut saved.katago),
        ("model", &mut saved.model),
        ("config", &mut saved.config),
        ("human_model", &mut saved.human_model),
    ] {
        if let Some(v) = body.get(key) {
            let t = v.as_str().unwrap_or("").trim();
            if t.is_empty() {
                *slot = None;
            } else {
                let p = PathBuf::from(t.replace('~', &std::env::var("HOME").unwrap_or_default()));
                if !p.exists() {
                    problems.push(format!("{}: {} does not exist", key, p.display()));
                }
                *slot = Some(p);
            }
        }
    }
    if !problems.is_empty() {
        return (StatusCode::BAD_REQUEST, problems.join("; ")).into_response();
    }
    if let Some(dir) = s.settings_path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    match serde_json::to_string_pretty(&saved).map_err(|e| e.to_string()).and_then(|t| std::fs::write(&s.settings_path, t).map_err(|e| e.to_string())) {
        Ok(()) => {
            tracing::info!("settings saved to {}", s.settings_path.display());
            Json(serde_json::json!({ "saved": saved, "path": s.settings_path, "note": "Restarting the engine with the new files." })).into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    }
}

/// Re-resolve engine paths (settings file, KaTrain, built-in) and restart KataGo.
async fn restart_engine(State(s): State<Shared>) -> Response {
    let running = s.jobs.lock().unwrap().values().filter(|j| matches!(j.state, JobState::Running { .. } | JobState::Queued)).count();
    if running > 0 {
        return (StatusCode::CONFLICT, format!("{} analysis job(s) are running; cancel them first", running)).into_response();
    }
    crate::start_engine(s.clone(), tokio::runtime::Handle::current(), false);
    StatusCode::NO_CONTENT.into_response()
}

async fn quit(State(s): State<Shared>) -> StatusCode {
    tracing::info!("quit requested from the web UI");
    for j in s.jobs.lock().unwrap().values() {
        j.cancel.cancel();
    }
    s.shutdown.cancel();
    StatusCode::NO_CONTENT
}

fn open_with_finder(args: &[&str]) -> Response {
    match std::process::Command::new("open").args(args).status() {
        Ok(st) if st.success() => StatusCode::NO_CONTENT.into_response(),
        Ok(st) => (StatusCode::INTERNAL_SERVER_ERROR, format!("open exited with {}", st)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn open_out_dir(State(s): State<Shared>) -> Response {
    let _ = std::fs::create_dir_all(&s.out_dir);
    open_with_finder(&[&s.out_dir.to_string_lossy()])
}

fn report_paths(s: &Shared, id: u64) -> Option<(String, String)> {
    let jobs = s.jobs.lock().unwrap();
    match &jobs.get(&id)?.state {
        JobState::Done { report_path, json_path, .. } => Some((report_path.clone(), json_path.clone())),
        _ => None,
    }
}

/// Open the finished report (or its JSON with `?which=json`) in the user's default application.
async fn open_report(
    State(s): State<Shared>,
    Path(id): Path<u64>,
    axum::extract::Query(q): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Response {
    match report_paths(&s, id) {
        Some((md, json)) => {
            let path = if q.get("which").map(|w| w == "json").unwrap_or(false) { json } else { md };
            open_with_finder(&[&path])
        }
        None => (StatusCode::NOT_FOUND, "report not ready").into_response(),
    }
}

async fn reveal_report(State(s): State<Shared>, Path(id): Path<u64>) -> Response {
    match report_paths(&s, id) {
        Some((md, _)) => open_with_finder(&["-R", &md]),
        None => (StatusCode::NOT_FOUND, "report not ready").into_response(),
    }
}

pub const HUMAN_PROFILES: &[&str] = &[
    "rank_20k", "rank_15k", "rank_10k", "rank_8k", "rank_5k", "rank_3k", "rank_1k",
    "rank_1d", "rank_2d", "rank_3d", "rank_5d", "rank_7d", "rank_9d",
    "preaz_5k", "preaz_1d", "preaz_5d", "preaz_9d", "proyear_1950", "proyear_2000", "proyear_2020",
];

// ------------------------------------------------------------------ files on disk

#[derive(Serialize)]
struct FileEntry {
    name: String,
    size: u64,
    modified: String,
    kind: String,
}

fn list_out_dir(out_dir: &std::path::Path) -> Vec<FileEntry> {
    let mut out = Vec::new();
    if let Ok(rd) = std::fs::read_dir(out_dir) {
        for e in rd.flatten() {
            let path = e.path();
            let Some(ext) = path.extension().and_then(|x| x.to_str()) else { continue };
            if ext != "md" && ext != "json" {
                continue;
            }
            let Ok(meta) = e.metadata() else { continue };
            if !meta.is_file() {
                continue;
            }
            let modified = meta
                .modified()
                .ok()
                .map(|t| chrono::DateTime::<chrono::Local>::from(t).to_rfc3339())
                .unwrap_or_default();
            out.push(FileEntry {
                name: e.file_name().to_string_lossy().to_string(),
                size: meta.len(),
                modified,
                kind: ext.to_string(),
            });
        }
    }
    out.sort_by(|a, b| b.modified.cmp(&a.modified));
    out
}

/// Only plain file names inside the output directory, only report extensions.
fn safe_file(out_dir: &std::path::Path, name: &str) -> Option<PathBuf> {
    if name.contains('/') || name.contains('\\') || name.starts_with('.') || name.is_empty() {
        return None;
    }
    if !(name.ends_with(".json") || name.ends_with(".md")) {
        return None;
    }
    let p = out_dir.join(name);
    if p.is_file() { Some(p) } else { None }
}

async fn list_files(State(s): State<Shared>) -> Json<serde_json::Value> {
    let files = list_out_dir(&s.out_dir);
    let json_bytes: u64 = files.iter().filter(|f| f.kind == "json").map(|f| f.size).sum();
    let json_count = files.iter().filter(|f| f.kind == "json").count();
    Json(serde_json::json!({ "out_dir": s.out_dir, "files": files, "json_count": json_count, "json_bytes": json_bytes }))
}

async fn delete_file(State(s): State<Shared>, Path(name): Path<String>) -> Response {
    match safe_file(&s.out_dir, &name) {
        Some(p) => match std::fs::remove_file(&p) {
            Ok(()) => {
                tracing::info!("deleted {}", p.display());
                StatusCode::NO_CONTENT.into_response()
            }
            Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
        },
        None => (StatusCode::NOT_FOUND, "no such report file").into_response(),
    }
}

/// Delete every JSON dump in the output directory (the Markdown reports stay).
async fn gc_files(State(s): State<Shared>) -> Json<serde_json::Value> {
    let mut count = 0u64;
    let mut bytes = 0u64;
    for f in list_out_dir(&s.out_dir).into_iter().filter(|f| f.kind == "json") {
        if std::fs::remove_file(s.out_dir.join(&f.name)).is_ok() {
            count += 1;
            bytes += f.size;
        }
    }
    tracing::info!("garbage-collected {} JSON files ({} bytes)", count, bytes);
    Json(serde_json::json!({ "deleted": count, "bytes": bytes }))
}

/// Load a saved JSON analysis back into the session as a finished job.
async fn load_file(State(s): State<Shared>, Path(name): Path<String>) -> Response {
    let Some(path) = safe_file(&s.out_dir, &name) else { return (StatusCode::NOT_FOUND, "no such file").into_response() };
    if !name.ends_with(".json") {
        return (StatusCode::BAD_REQUEST, "only JSON analyses can be loaded").into_response();
    }
    let analysis: GameAnalysis = match std::fs::read(&path).map_err(|e| e.to_string()).and_then(|b| serde_json::from_slice(&b).map_err(|e| e.to_string())) {
        Ok(a) => a,
        Err(e) => return (StatusCode::BAD_REQUEST, format!("cannot read {}: {}", name, e)).into_response(),
    };
    let md_path = path.with_extension("md");
    let md = if md_path.is_file() {
        std::fs::read_to_string(&md_path).unwrap_or_else(|_| render_markdown(&analysis))
    } else {
        render_markdown(&analysis)
    };
    let id = {
        let mut n = s.next_job.lock().unwrap();
        *n += 1;
        *n
    };
    let stem = std::path::Path::new(&name).file_stem().and_then(|x| x.to_str()).unwrap_or("game");
    let job = Job {
        id,
        file_name: format!("{} (loaded)", stem),
        created_at: chrono::Local::now().to_rfc3339(),
        max_visits: analysis.options.max_visits,
        human_profile: analysis.options.human_profile.clone(),
        state: JobState::Done {
            report_path: if md_path.is_file() { md_path.display().to_string() } else { String::new() },
            json_path: path.display().to_string(),
            summary: summarize(&analysis),
        },
        cancel: CancellationToken::new(),
        markdown: Some(Arc::new(md)),
        json: None,
        game: Arc::new(analysis.game.clone()),
        live: analysis.turns.iter().map(|t| Some(LiveTurn::from(t))).collect(),
        turns_done: analysis.turns.len(),
        turns_total: analysis.turns.len(),
    };
    s.jobs.lock().unwrap().insert(id, job);
    Json(serde_json::json!({ "job_id": id })).into_response()
}

/// Which turns have been analysed so far.
async fn live_summary(State(s): State<Shared>, Path(id): Path<u64>) -> Response {
    let jobs = s.jobs.lock().unwrap();
    let Some(j) = jobs.get(&id) else { return (StatusCode::NOT_FOUND, "no such job").into_response() };
    let done: Vec<usize> = j.live.iter().enumerate().filter_map(|(i, t)| t.as_ref().map(|_| i)).collect();
    let latest = done.last().copied();
    let series: Vec<serde_json::Value> = j
        .live
        .iter()
        .enumerate()
        .filter_map(|(i, t)| t.as_ref().map(|t| serde_json::json!([i, t.winrate, t.score_lead])))
        .collect();
    Json(serde_json::json!({
        "id": id,
        "total": j.turns_total,
        "done": done.len(),
        "latest": latest,
        "series": series,
        "state": j.state,
        "size_x": j.game.size_x,
        "size_y": j.game.size_y,
        "moves": j.game.moves.len(),
    }))
    .into_response()
}

/// Board position after `turn` moves plus KataGo's evaluation of it (if analysed yet).
async fn live_turn(State(s): State<Shared>, Path((id, turn)): Path<(u64, usize)>) -> Response {
    let (game, eval) = {
        let jobs = s.jobs.lock().unwrap();
        let Some(j) = jobs.get(&id) else { return (StatusCode::NOT_FOUND, "no such job").into_response() };
        if turn > j.game.moves.len() {
            return (StatusCode::NOT_FOUND, "no such turn").into_response();
        }
        (j.game.clone(), j.live.get(turn).cloned().flatten())
    };
    let mut board = Board::new(game.size_x, game.size_y);
    for c in &game.setup_black {
        board.set(*c, Some(Color::Black));
    }
    for c in &game.setup_white {
        board.set(*c, Some(Color::White));
    }
    for m in game.moves.iter().take(turn) {
        if let Some(p) = m.point {
            board.play(m.color, p);
        }
    }
    let rows: Vec<String> = (0..game.size_y)
        .map(|y| {
            (0..game.size_x)
                .map(|x| match board.get(crate::sgf::Coord { x, y }) {
                    Some(Color::Black) => 'X',
                    Some(Color::White) => 'O',
                    None => '.',
                })
                .collect()
        })
        .collect();
    let mv_json = |m: &crate::sgf::Move| {
        serde_json::json!({
            "color": m.color,
            "move": m.point.map(|p| p.to_gtp(game.size_y)).unwrap_or_else(|| "pass".into()),
        })
    };
    let last_move = if turn > 0 { game.moves.get(turn - 1).map(mv_json) } else { None };
    let next_move = game.moves.get(turn).map(mv_json);
    Json(serde_json::json!({
        "turn": turn,
        "size_x": game.size_x,
        "size_y": game.size_y,
        "board": rows,
        "last_move": last_move,
        "next_move": next_move,
        "eval": eval,
    }))
    .into_response()
}

/// The finished report rendered as HTML.
async fn report_page(State(s): State<Shared>, Path(id): Path<u64>) -> Response {
    let (md, name) = {
        let jobs = s.jobs.lock().unwrap();
        let Some(j) = jobs.get(&id) else { return (StatusCode::NOT_FOUND, "no such job").into_response() };
        match &j.markdown {
            Some(md) => (md.clone(), j.file_name.clone()),
            None => return (StatusCode::CONFLICT, "report not ready").into_response(),
        }
    };
    let html = crate::report_html::render_page(&name, &md, id);
    Html(html).into_response()
}

async fn list_jobs(State(s): State<Shared>) -> Json<Vec<Job>> {
    let jobs = s.jobs.lock().unwrap();
    Json(jobs.values().rev().cloned().collect())
}

async fn get_job(State(s): State<Shared>, Path(id): Path<u64>) -> Response {
    let jobs = s.jobs.lock().unwrap();
    match jobs.get(&id) {
        Some(j) => Json(j.clone()).into_response(),
        None => (StatusCode::NOT_FOUND, "no such job").into_response(),
    }
}

async fn cancel_all(State(s): State<Shared>) -> StatusCode {
    for j in s.jobs.lock().unwrap().values() {
        if matches!(j.state, JobState::Queued | JobState::Running { .. }) {
            j.cancel.cancel();
        }
    }
    StatusCode::NO_CONTENT
}

async fn cancel_job(State(s): State<Shared>, Path(id): Path<u64>) -> Response {
    let jobs = s.jobs.lock().unwrap();
    match jobs.get(&id) {
        Some(j) => {
            j.cancel.cancel();
            StatusCode::NO_CONTENT.into_response()
        }
        None => (StatusCode::NOT_FOUND, "no such job").into_response(),
    }
}

fn serve_text(job: Option<&Job>, pick: fn(&Job) -> Option<Arc<String>>, mime: &str, ext: &str, inline: bool) -> Response {
    let job = match job {
        Some(j) => j,
        None => return (StatusCode::NOT_FOUND, "no such job").into_response(),
    };
    match pick(job) {
        Some(text) => {
            let base = sanitize(&job.file_name);
            let disposition = if inline {
                "inline".to_string()
            } else {
                format!("attachment; filename=\"{}_review.{}\"", base, ext)
            };
            let content_type = if inline { "text/plain; charset=utf-8" } else { mime };
            Response::builder()
                .header(header::CONTENT_TYPE, content_type)
                .header(header::CONTENT_DISPOSITION, disposition)
                .body(Body::from(text.as_str().to_owned()))
                .unwrap()
        }
        None => (StatusCode::CONFLICT, "report not ready").into_response(),
    }
}

async fn report_md(
    State(s): State<Shared>,
    Path(id): Path<u64>,
    axum::extract::Query(q): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Response {
    let jobs = s.jobs.lock().unwrap();
    serve_text(jobs.get(&id), |j| j.markdown.clone(), "text/markdown; charset=utf-8", "md", q.contains_key("inline"))
}

async fn report_json(
    State(s): State<Shared>,
    Path(id): Path<u64>,
    axum::extract::Query(q): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Response {
    let jobs = s.jobs.lock().unwrap();
    serve_text(jobs.get(&id), |j| j.json.clone(), "application/json", "json", q.contains_key("inline"))
}

fn sanitize(name: &str) -> String {
    let stem = std::path::Path::new(name)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("game");
    let cleaned: String = stem
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect();
    if cleaned.is_empty() {
        "game".to_string()
    } else {
        cleaned.chars().take(60).collect()
    }
}

async fn analyze(State(s): State<Shared>, mut multipart: Multipart) -> Response {
    let mut sgf_bytes: Option<Vec<u8>> = None;
    let mut file_name = "game.sgf".to_string();
    let mut max_visits: Option<u64> = None;
    let mut human_profile: Option<String> = None;
    let mut student: Option<Color> = None;
    let mut submitted: Vec<(String, Vec<u8>)> = Vec::new();

    while let Ok(Some(field)) = multipart.next_field().await {
        let name = field.name().unwrap_or("").to_string();
        match name.as_str() {
            "sgf" => {
                if let Some(f) = field.file_name() {
                    file_name = f.to_string();
                }
                match field.bytes().await {
                    Ok(b) => {
                        if !b.is_empty() {
                            submitted.push((file_name.clone(), b.to_vec()));
                        }
                    }
                    Err(e) => return (StatusCode::BAD_REQUEST, format!("upload failed: {}", e)).into_response(),
                }
            }
            "student" => {
                if let Ok(t) = field.text().await {
                    student = match t.trim().to_ascii_uppercase().as_str() {
                        "B" => Some(Color::Black),
                        "W" => Some(Color::White),
                        _ => None,
                    };
                }
            }
            "human_profile" => {
                if let Ok(t) = field.text().await {
                    let t = t.trim().to_string();
                    if !t.is_empty() {
                        if !t.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                            return (StatusCode::BAD_REQUEST, "invalid human profile").into_response();
                        }
                        if s.engine_config.read().unwrap().human_model.is_none() {
                            return (StatusCode::BAD_REQUEST, "human profile requested but no human model is loaded").into_response();
                        }
                        human_profile = Some(t);
                    }
                }
            }
            "visits" => {
                if let Ok(t) = field.text().await {
                    let t = t.trim();
                    if !t.is_empty() {
                        match t.parse::<u64>() {
                            Ok(v) if v > 0 => max_visits = Some(v),
                            _ => return (StatusCode::BAD_REQUEST, "visits must be a positive integer").into_response(),
                        }
                    }
                }
            }
            _ => {}
        }
    }
    if submitted.is_empty() {
        sgf_bytes = None;
    }
    let _ = sgf_bytes;
    if submitted.is_empty() {
        return (StatusCode::BAD_REQUEST, "no SGF file in upload").into_response();
    }
    if s.engine().is_none() {
        return (StatusCode::SERVICE_UNAVAILABLE, "KataGo is still starting (or failed to start); try again in a moment").into_response();
    }

    let mut ids = Vec::new();
    for (name, bytes) in submitted {
        match start_job(&s, name, bytes, max_visits, human_profile.clone(), student) {
            Ok(id) => ids.push(id),
            Err(e) => return (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
        }
    }
    Json(serde_json::json!({ "job_ids": ids })).into_response()
}

pub fn start_job(s: &Shared, file_name: String, bytes: Vec<u8>, max_visits: Option<u64>, human_profile: Option<String>, student: Option<Color>) -> Result<u64> {
    let engine = s.engine().ok_or_else(|| anyhow::anyhow!("KataGo is not running"))?;
    let text = String::from_utf8_lossy(&bytes).to_string();
    let game = parse_game(&text).map_err(|e| anyhow::anyhow!("{}: {}", file_name, e))?;

    let id = {
        let mut n = s.next_job.lock().unwrap();
        *n += 1;
        *n
    };
    let cancel = CancellationToken::new();
    let job = Job {
        id,
        file_name: file_name.clone(),
        created_at: chrono::Local::now().to_rfc3339(),
        max_visits,
        human_profile: human_profile.clone(),
        state: JobState::Queued,
        cancel: cancel.clone(),
        markdown: None,
        json: None,
        game: Arc::new(game.clone()),
        live: vec![None; game.moves.len() + 1],
        turns_done: 0,
        turns_total: game.moves.len() + 1,
    };
    s.jobs.lock().unwrap().insert(id, job);

    let state = s.clone();
    tokio::spawn(async move {
        // Wait for a slot; cancelling while queued just marks the job cancelled.
        let permit = tokio::select! {
            p = state.job_slots.clone().acquire_owned() => p,
            _ = cancel.cancelled() => {
                if let Some(j) = state.jobs.lock().unwrap().get_mut(&id) {
                    j.state = JobState::Cancelled;
                }
                return;
            }
        };
        let Ok(_permit) = permit else { return };
        let opts = AnalysisOptions {
            max_visits,
            human_profile,
            student,
            ..Default::default()
        };
        let st = state.clone();
        let progress = move |done: usize, total: usize, turn: Option<&TurnEval>| {
            if let Some(j) = st.jobs.lock().unwrap().get_mut(&id) {
                j.state = JobState::Running { done, total };
                j.turns_done = done;
                if let Some(t) = turn {
                    if let Some(slot) = j.live.get_mut(t.turn) {
                        *slot = Some(LiveTurn::from(t));
                    }
                }
            }
        };
        let result = analyze_game(&engine, game, opts, cancel.clone(), progress).await;
        let mut jobs = state.jobs.lock().unwrap();
        let job = match jobs.get_mut(&id) {
            Some(j) => j,
            None => return,
        };
        match result {
            Ok(analysis) => match finish(&state.out_dir, &file_name, &analysis) {
                Ok((md, json, md_path, json_path)) => {
                    job.markdown = Some(Arc::new(md));
                    job.json = Some(Arc::new(json));
                    job.state = JobState::Done {
                        report_path: md_path,
                        json_path,
                        summary: summarize(&analysis),
                    };
                }
                Err(e) => job.state = JobState::Failed { error: e.to_string() },
            },
            Err(e) if cancel.is_cancelled() => {
                tracing::info!("job {} cancelled: {}", id, e);
                job.state = JobState::Cancelled;
            }
            Err(e) => job.state = JobState::Failed { error: e.to_string() },
        }
    });
    Ok(id)
}

pub fn write_outputs(out_dir: &std::path::Path, base: &str, analysis: &GameAnalysis) -> Result<(String, String, PathBuf, PathBuf)> {
    std::fs::create_dir_all(out_dir)?;
    let md = render_markdown(analysis);
    // Compact: the per-position ownership/policy arrays would triple the size when pretty-printed.
    let json = serde_json::to_string(analysis)?;
    let md_path = out_dir.join(format!("{}.md", base));
    let json_path = out_dir.join(format!("{}.json", base));
    std::fs::write(&md_path, &md)?;
    std::fs::write(&json_path, &json)?;
    Ok((md, json, md_path, json_path))
}

fn finish(out_dir: &std::path::Path, file_name: &str, analysis: &GameAnalysis) -> Result<(String, String, String, String)> {
    let stamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
    let base = format!("{}_{}", stamp, sanitize(file_name));
    let (md, json, md_path, json_path) = write_outputs(out_dir, &base, analysis)?;
    tracing::info!("wrote {}", md_path.display());
    Ok((md, json, md_path.display().to_string(), json_path.display().to_string()))
}

fn summarize(a: &GameAnalysis) -> JobSummary {
    let mean = |c: crate::sgf::Color| {
        let v: Vec<f64> = a.reviews.iter().filter(|r| r.color == c).map(|r| r.point_loss.max(0.0)).collect();
        if v.is_empty() {
            0.0
        } else {
            v.iter().sum::<f64>() / v.len() as f64
        }
    };
    JobSummary {
        moves: a.game.moves.len(),
        black: a.game.player_black.clone().unwrap_or_else(|| "Black".into()),
        white: a.game.player_white.clone().unwrap_or_else(|| "White".into()),
        result: a.game.result.clone(),
        final_winrate_black: a.turns.last().map(|t| t.winrate).unwrap_or(0.5),
        final_score_lead: a.turns.last().map(|t| t.score_lead).unwrap_or(0.0),
        elapsed_seconds: a.elapsed_seconds,
        black_mean_loss: mean(crate::sgf::Color::Black),
        white_mean_loss: mean(crate::sgf::Color::White),
    }
}
