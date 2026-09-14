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

#[derive(Debug, Clone, Serialize, serde::Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum JobState {
    Queued,
    Running { done: usize, total: usize },
    Done { report_path: String, json_path: String, summary: JobSummary },
    Failed { error: String },
    Cancelled,
}

#[derive(Debug, Clone, Serialize, serde::Deserialize)]
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
    /// True once the position was re-analysed by the deep second pass.
    #[serde(default)]
    pub deepened: bool,
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
            deepened: false,
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
    #[serde(default)]
    pub human_profile_target: Option<String>,
    #[serde(default)]
    pub student: Option<Color>,
    #[serde(default)]
    pub two_pass: bool,
    #[serde(default)]
    pub deep_visits: Option<u64>,
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
    /// Positions analysed so far (kept so a cancelled or failed run can resume).
    #[serde(skip)]
    pub partial: Vec<Option<TurnEval>>,
    #[serde(skip)]
    pub attempts: u32,
    #[serde(skip)]
    pub last_access: Option<std::time::Instant>,
    /// True when the position data was dropped from memory and must be reloaded from the JSON dump.
    #[serde(skip)]
    pub spilled: bool,
    /// What the analysis is doing right now ("pass 2 of 2: ...").
    #[serde(default)]
    pub phase: String,
    /// The position most recently analysed or re-analysed (what the live view follows).
    #[serde(default)]
    pub last_updated_turn: Option<usize>,
}

impl Job {
    fn touch(&mut self) {
        self.last_access = Some(std::time::Instant::now());
    }

    fn paths(&self) -> Option<(String, String)> {
        match &self.state {
            JobState::Done { report_path, json_path, .. } => Some((report_path.clone(), json_path.clone())),
            _ => None,
        }
    }
}

/// Persisted form of the job list (finished jobs only; running ones cannot survive a restart).
#[derive(Serialize, serde::Deserialize)]
struct SavedJob {
    id: u64,
    file_name: String,
    created_at: String,
    max_visits: Option<u64>,
    human_profile: Option<String>,
    #[serde(default)]
    student: Option<Color>,
    #[serde(default)]
    two_pass: bool,
    #[serde(default)]
    deep_visits: Option<u64>,
    state: serde_json::Value,
    turns_total: usize,
}

pub fn persist_jobs(s: &AppState) {
    let jobs = s.jobs.lock().unwrap();
    let saved: Vec<SavedJob> = jobs
        .values()
        .filter(|j| matches!(j.state, JobState::Done { .. } | JobState::Failed { .. } | JobState::Cancelled))
        .map(|j| SavedJob {
            id: j.id,
            file_name: j.file_name.clone(),
            created_at: j.created_at.clone(),
            max_visits: j.max_visits,
            human_profile: j.human_profile.clone(),
            student: j.student,
            two_pass: j.two_pass,
            deep_visits: j.deep_visits,
            state: serde_json::to_value(&j.state).unwrap_or(serde_json::Value::Null),
            turns_total: j.turns_total,
        })
        .collect();
    let path = s.settings_path.with_file_name("jobs.json");
    if let Ok(text) = serde_json::to_string(&saved) {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(path, text);
    }
}

/// Recreate finished jobs from jobs.json; position data is loaded lazily from the JSON dumps.
pub fn restore_jobs(s: &AppState) {
    let path = s.settings_path.with_file_name("jobs.json");
    let Ok(text) = std::fs::read_to_string(&path) else { return };
    let Ok(saved) = serde_json::from_str::<Vec<SavedJob>>(&text) else { return };
    let mut jobs = s.jobs.lock().unwrap();
    let mut max_id = *s.next_job.lock().unwrap();
    let mut kept = 0;
    for sj in saved.into_iter().rev().take(50) {
        let Ok(state) = serde_json::from_value::<JobState>(sj.state.clone()) else { continue };
        if let JobState::Done { json_path, .. } = &state {
            if !std::path::Path::new(json_path).is_file() {
                continue; // the dump was deleted; nothing to show
            }
        }
        max_id = max_id.max(sj.id);
        jobs.insert(
            sj.id,
            Job {
                id: sj.id,
                file_name: sj.file_name,
                created_at: sj.created_at,
                max_visits: sj.max_visits,
                human_profile: sj.human_profile,
                human_profile_target: None,
                student: sj.student,
                two_pass: sj.two_pass,
                deep_visits: sj.deep_visits,
                state,
                cancel: CancellationToken::new(),
                markdown: None,
                json: None,
                game: Arc::new(GameRecord::default()),
                live: Vec::new(),
                turns_done: sj.turns_total,
                turns_total: sj.turns_total,
                partial: Vec::new(),
                attempts: 0,
                last_access: None,
                spilled: true,
                phase: String::new(),
                last_updated_turn: None,
            },
        );
        kept += 1;
    }
    *s.next_job.lock().unwrap() = max_id;
    tracing::info!("restored {} finished jobs from {}", kept, path.display());
}

/// Load position data for a spilled or restored Done job from its JSON dump.
fn ensure_loaded(s: &AppState, id: u64) {
    let json_path = {
        let mut jobs = s.jobs.lock().unwrap();
        let Some(j) = jobs.get_mut(&id) else { return };
        j.touch();
        if !j.spilled {
            return;
        }
        match j.paths() {
            Some((_, jp)) => jp,
            None => return,
        }
    };
    let Ok(bytes) = std::fs::read(&json_path) else { return };
    let Ok(analysis) = serde_json::from_slice::<GameAnalysis>(&bytes) else { return };
    let md_path = std::path::Path::new(&json_path).with_extension("md");
    let md = std::fs::read_to_string(&md_path).unwrap_or_else(|_| render_markdown(&analysis));
    let mut jobs = s.jobs.lock().unwrap();
    if let Some(j) = jobs.get_mut(&id) {
        j.game = Arc::new(analysis.game.clone());
        j.live = analysis.turns.iter().map(|t| Some(LiveTurn::from(t))).collect();
        j.markdown = Some(Arc::new(md));
        j.turns_total = analysis.turns.len();
        j.turns_done = analysis.turns.len();
        j.spilled = false;
    }
}

/// Drop position data of finished jobs nobody has looked at for a while (reloaded on demand).
pub fn spill_idle_jobs(s: &AppState, idle: std::time::Duration) {
    let mut jobs = s.jobs.lock().unwrap();
    for j in jobs.values_mut() {
        if !matches!(j.state, JobState::Done { .. }) || j.spilled || j.paths().is_none() {
            continue;
        }
        let idle_for = j.last_access.map(|t| t.elapsed()).unwrap_or(idle);
        if idle_for >= idle {
            j.live = Vec::new();
            j.markdown = None;
            j.json = None;
            j.partial = Vec::new();
            j.game = Arc::new(GameRecord::default());
            j.spilled = true;
        }
    }
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
    /// Output of the last `katago benchmark` run, and whether it is still running.
    pub benchmark: Mutex<(bool, String)>,
}

#[derive(serde::Deserialize)]
struct ExploreRequest {
    turn: usize,
    /// Extra moves (GTP) played after `turn`, alternating colours from the side to move.
    #[serde(default)]
    moves: Vec<String>,
    #[serde(default)]
    visits: Option<u64>,
}

/// Analyse a hypothetical continuation from a position of a job's game: one KataGo query,
/// outside the job queue (single position, seconds).
async fn explore(State(s): State<Shared>, Path(id): Path<u64>, Json(req): Json<ExploreRequest>) -> Response {
    ensure_loaded(&s, id);
    let Some(engine) = s.engine() else { return (StatusCode::SERVICE_UNAVAILABLE, "KataGo is not running").into_response() };
    let (game, human) = {
        let jobs = s.jobs.lock().unwrap();
        let Some(j) = jobs.get(&id) else { return (StatusCode::NOT_FOUND, "no such job").into_response() };
        (j.game.clone(), j.human_profile.clone())
    };
    if req.turn > game.moves.len() || req.moves.len() > 40 {
        return (StatusCode::BAD_REQUEST, "bad turn or too many moves").into_response();
    }
    // Build a game record truncated at `turn` plus the variation.
    let mut g = (*game).clone();
    g.moves.truncate(req.turn);
    let mut color = g.moves.last().map(|m| m.color.opponent()).unwrap_or(g.who_moves_first());
    if req.turn == 0 {
        color = g.who_moves_first();
    }
    for mv in &req.moves {
        let point = if mv == "pass" {
            None
        } else {
            match crate::sgf::Coord::from_gtp(mv, g.size_y) {
                Some(c) => Some(c),
                None => return (StatusCode::BAD_REQUEST, format!("bad coordinate {}", mv)).into_response(),
            }
        };
        g.moves.push(crate::sgf::Move { color, point, comment: None, time_left: None });
        color = color.opponent();
    }
    let opts = AnalysisOptions { max_visits: Some(req.visits.unwrap_or(400).min(5000)), human_profile: human, ..Default::default() };
    let rules = crate::analysis::katago_rules(g.rules.as_deref());
    let komi = g.komi.unwrap_or_else(|| crate::analysis::default_komi(&rules));
    let last = g.moves.len();
    match crate::analysis::analyze_positions(&engine, &g, &rules, komi, &opts, &[last]).await {
        Ok(mut turns) if !turns.is_empty() => {
            let t = turns.remove(0);
            let mut board = Board::new(g.size_x, g.size_y);
            for c in &g.setup_black {
                board.set(*c, Some(Color::Black));
            }
            for c in &g.setup_white {
                board.set(*c, Some(Color::White));
            }
            for m in &g.moves {
                if let Some(p) = m.point {
                    board.play(m.color, p);
                }
            }
            let rows: Vec<String> = (0..g.size_y)
                .map(|y| (0..g.size_x).map(|x| match board.get(crate::sgf::Coord { x, y }) { Some(Color::Black) => 'X', Some(Color::White) => 'O', None => '.' }).collect())
                .collect();
            let last_move = g.moves.last().map(|m| serde_json::json!({"color": m.color, "move": m.point.map(|p| p.to_gtp(g.size_y)).unwrap_or_else(|| "pass".into())}));
            Json(serde_json::json!({
                "turn": req.turn, "variation": req.moves, "size_x": g.size_x, "size_y": g.size_y,
                "board": rows, "last_move": last_move, "next_move": serde_json::Value::Null,
                "eval": LiveTurn::from(&t),
            }))
            .into_response()
        }
        Ok(_) => (StatusCode::INTERNAL_SERVER_ERROR, "no result").into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
    }
}

/// Run `katago benchmark` (6 positions at 300 visits) in the background.
async fn benchmark_start(State(s): State<Shared>) -> Response {
    {
        let running = s.jobs.lock().unwrap().values().any(|j| matches!(j.state, JobState::Running { .. } | JobState::Queued));
        if running {
            return (StatusCode::CONFLICT, "analysis jobs are running; benchmark later").into_response();
        }
        let mut b = s.benchmark.lock().unwrap();
        if b.0 {
            return (StatusCode::CONFLICT, "benchmark already running").into_response();
        }
        *b = (true, String::new());
    }
    let cfg = s.engine_config.read().unwrap().clone();
    let st = s.clone();
    let bench_cfg = s.settings_path.with_file_name("benchmark.cfg");
    tokio::task::spawn_blocking(move || {
        // `katago benchmark` needs GTP-style keys (numSearchThreads); derive a benchmark config
        // from the analysis config, keeping its backend/device settings.
        let base = std::fs::read_to_string(&cfg.config).unwrap_or_default();
        let per_thread = base
            .lines()
            .find_map(|l| l.trim().strip_prefix("numSearchThreadsPerAnalysisThread").and_then(|r| r.trim().trim_start_matches('=').trim().split_whitespace().next()).and_then(|v| v.parse::<u32>().ok()))
            .unwrap_or(4);
        let analysis_threads = base
            .lines()
            .find_map(|l| l.trim().strip_prefix("numAnalysisThreads").and_then(|r| r.trim().trim_start_matches('=').trim().split_whitespace().next()).and_then(|v| v.parse::<u32>().ok()))
            .unwrap_or(2);
        let threads = (per_thread * analysis_threads).clamp(1, 64);
        let filtered: Vec<&str> = base.lines().filter(|l| !l.trim_start().starts_with("numSearchThreads ") && !l.trim_start().starts_with("numSearchThreads=")).collect();
        let text = format!("{}\nnumSearchThreads = {}\n", filtered.join("\n"), threads);
        if std::fs::write(&bench_cfg, text).is_err() {
            *st.benchmark.lock().unwrap() = (false, "could not write benchmark config".into());
            return;
        }
        let out = std::process::Command::new(&cfg.katago)
            .args(["benchmark", "-model"])
            .arg(&cfg.model)
            .arg("-config")
            .arg(&bench_cfg)
            .args(["-v", "300", "-n", "6", "-t"])
            .arg(threads.to_string())
            .output();
        let text = match out {
            Ok(o) => {
                let mut t = String::from_utf8_lossy(&o.stdout).to_string();
                if !o.status.success() {
                    t.push_str(&String::from_utf8_lossy(&o.stderr));
                }
                t
            }
            Err(e) => format!("could not run katago benchmark: {}", e),
        };
        // Keep the informative lines only.
        let summary: Vec<&str> = text
            .lines()
            .filter(|l| l.contains("visits/s") || l.contains("backend") || l.contains("Model name") || l.contains("numSearchThreads") || l.contains("nnEvals") || l.contains("Ordered summary") || l.contains("Elo") || l.contains("Error") || l.contains("error"))
            .collect();
        let result = if summary.is_empty() { text.chars().rev().take(1500).collect::<String>().chars().rev().collect() } else { summary.join("\n") };
        *st.benchmark.lock().unwrap() = (false, result);
    });
    StatusCode::ACCEPTED.into_response()
}

async fn benchmark_status(State(s): State<Shared>) -> Json<serde_json::Value> {
    let b = s.benchmark.lock().unwrap();
    Json(serde_json::json!({ "running": b.0, "output": b.1 }))
}

/// Zip the report, the JSON dump and (when the skill folder is configured) the parsed JSON the
/// skill's parser produces, then reveal the zip in Finder.
async fn lesson_bundle(State(s): State<Shared>, Path(id): Path<u64>) -> Response {
    let Some((md, json)) = ({
        let jobs = s.jobs.lock().unwrap();
        jobs.get(&id).and_then(|j| j.paths())
    }) else { return (StatusCode::CONFLICT, "report not ready").into_response() };
    let md_path = PathBuf::from(&md);
    let stem = md_path.file_stem().map(|x| x.to_string_lossy().to_string()).unwrap_or_else(|| "report".into());
    let dir = s.out_dir.join(format!("{}_bundle", stem));
    let _ = std::fs::remove_dir_all(&dir);
    if let Err(e) = std::fs::create_dir_all(&dir) {
        return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response();
    }
    let _ = std::fs::copy(&md, dir.join("report.md"));
    let _ = std::fs::copy(&json, dir.join("analysis.json"));
    let mut notes = vec!["report.md: the go_teacher review (the only input the lesson skill needs)".to_string(), "analysis.json: full per-position data".to_string()];
    if let Some(skill) = crate::load_settings().skill_dir.filter(|d| d.join("scripts/parse_review.py").is_file()) {
        let out = std::process::Command::new("python3")
            .arg(skill.join("scripts/parse_review.py"))
            .arg(dir.join("report.md"))
            .arg(dir.join("parsed.json"))
            .output();
        match out {
            Ok(o) if o.status.success() => notes.push("parsed.json: output of the skill's parse_review.py (brief)".into()),
            Ok(o) => notes.push(format!("parse_review.py failed: {}", String::from_utf8_lossy(&o.stderr).trim())),
            Err(e) => notes.push(format!("could not run parse_review.py: {}", e)),
        }
        let _ = std::fs::copy(skill.join("SKILL.md"), dir.join("SKILL.md"));
    } else {
        notes.push("set the lesson skill folder in Engine settings to include parsed.json and SKILL.md".into());
    }
    let _ = std::fs::write(dir.join("README.txt"), format!("Go Teacher lesson bundle\n\n{}\n\nUpload report.md (or the whole zip) to your teaching agent with the go-game-teacher skill.\n", notes.join("\n")));
    let zip_path = s.out_dir.join(format!("{}_bundle.zip", stem));
    let _ = std::fs::remove_file(&zip_path);
    let status = std::process::Command::new("zip").arg("-q").arg("-r").arg(&zip_path).arg(dir.file_name().unwrap()).current_dir(&s.out_dir).status();
    let _ = std::fs::remove_dir_all(&dir);
    match status {
        Ok(st) if st.success() => {
            let _ = std::process::Command::new("open").arg("-R").arg(&zip_path).status();
            (StatusCode::OK, format!("Lesson bundle written to {}\n{}", zip_path.display(), notes.join("\n"))).into_response()
        }
        _ => (StatusCode::INTERNAL_SERVER_ERROR, "zip failed").into_response(),
    }
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
        .route("/api/jobs/:id/resume", post(resume_job))
        .route("/api/jobs/:id/explore", post(explore))
        .route("/api/jobs/:id/lesson-bundle", post(lesson_bundle))
        .route("/api/engine/benchmark", get(benchmark_status).post(benchmark_start))
        .route("/api/jobs/:id/report.md", get(report_md))
        .route("/api/jobs/:id/detailed.md", get(report_detailed))
        .route("/api/jobs/:id/report.json", get(report_json))
        .route("/api/jobs/:id/live", get(live_summary))
        .route("/api/jobs/:id/turn/:turn", get(live_turn))
        .route("/report/:id", get(report_page))
        .route("/report/:id/detailed", get(report_detailed_page))
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
    if let Some(v) = body.get("watch_human_profile") {
        let t = v.as_str().unwrap_or("").trim().to_string();
        saved.watch_human_profile = if t.is_empty() || !t.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') { None } else { Some(t) };
    }
    for (key, slot) in [
        ("katago", &mut saved.katago),
        ("model", &mut saved.model),
        ("config", &mut saved.config),
        ("human_model", &mut saved.human_model),
        ("watch_dir", &mut saved.watch_dir),
        ("skill_dir", &mut saved.skill_dir),
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
        student: analysis.student,
        human_profile_target: analysis.options.human_profile_target.clone(),
        two_pass: analysis.options.two_pass,
        deep_visits: analysis.options.deep_visits,
        partial: Vec::new(),
        attempts: 0,
        last_access: Some(std::time::Instant::now()),
        spilled: false,
        phase: String::new(),
        last_updated_turn: None,
    };
    s.jobs.lock().unwrap().insert(id, job);
    Json(serde_json::json!({ "job_id": id })).into_response()
}

/// Which turns have been analysed so far.
async fn live_summary(State(s): State<Shared>, Path(id): Path<u64>) -> Response {
    ensure_loaded(&s, id);
    let jobs = s.jobs.lock().unwrap();
    let Some(j) = jobs.get(&id) else { return (StatusCode::NOT_FOUND, "no such job").into_response() };
    let done: Vec<usize> = j.live.iter().enumerate().filter_map(|(i, t)| t.as_ref().map(|_| i)).collect();
    let latest = j.last_updated_turn.or_else(|| done.last().copied());
    let deepened = j.live.iter().filter(|t| t.as_ref().map_or(false, |x| x.deepened)).count();
    let series: Vec<serde_json::Value> = j
        .live
        .iter()
        .enumerate()
        .filter_map(|(i, t)| t.as_ref().map(|t| serde_json::json!([i, t.winrate, t.score_lead])))
        .collect();
    // Point loss of move i (1-based) from consecutive analysed positions: [move, colour, loss].
    let losses: Vec<serde_json::Value> = (0..j.game.moves.len())
        .filter_map(|i| {
            let (Some(b), Some(a)) = (j.live.get(i).cloned().flatten(), j.live.get(i + 1).cloned().flatten()) else { return None };
            let sign = if j.game.moves[i].color == Color::Black { 1.0 } else { -1.0 };
            Some(serde_json::json!([i + 1, j.game.moves[i].color.letter(), sign * (b.score_lead - a.score_lead)]))
        })
        .collect();
    Json(serde_json::json!({
        "id": id,
        "total": j.turns_total,
        "done": j.turns_done.max(done.len()),
        "positions_analysed": done.len(),
        "deepened": deepened,
        "phase": j.phase,
        "latest": latest,
        "series": series,
        "losses": losses,
        "positions": j.game.moves.len() + 1,
        "state": j.state,
        "size_x": j.game.size_x,
        "size_y": j.game.size_y,
        "moves": j.game.moves.len(),
    }))
    .into_response()
}

/// Board position after `turn` moves plus KataGo's evaluation of it (if analysed yet).
async fn live_turn(State(s): State<Shared>, Path((id, turn)): Path<(u64, usize)>) -> Response {
    ensure_loaded(&s, id);
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
    ensure_loaded(&s, id);
    let (md, name) = {
        let jobs = s.jobs.lock().unwrap();
        let Some(j) = jobs.get(&id) else { return (StatusCode::NOT_FOUND, "no such job").into_response() };
        match &j.markdown {
            Some(md) => (md.clone(), j.file_name.clone()),
            None => return (StatusCode::CONFLICT, "report not ready").into_response(),
        }
    };
    let html = crate::report_html::render_page(&name, &md, id, crate::report_html::ReportKind::Teaching);
    Html(html).into_response()
}

/// The detailed report uses the same Markdown renderer and navigation as the teaching report.
async fn report_detailed_page(State(s): State<Shared>, Path(id): Path<u64>) -> Response {
    let name = {
        let jobs = s.jobs.lock().unwrap();
        let Some(job) = jobs.get(&id) else { return (StatusCode::NOT_FOUND, "no such job").into_response() };
        job.file_name.clone()
    };
    match detailed_markdown(&s, id) {
        Ok(md) => Html(crate::report_html::render_page(&name, &md, id, crate::report_html::ReportKind::Detailed)).into_response(),
        Err(error) => error.into_response(),
    }
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
    // Spilled jobs: read the file from disk instead.
    let from_disk = if pick(job).is_none() {
        job.paths().and_then(|(md, js)| std::fs::read_to_string(if ext == "json" { js } else { md }).ok()).map(Arc::new)
    } else {
        None
    };
    match pick(job).or(from_disk) {
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

fn detailed_markdown(s: &Shared, id: u64) -> std::result::Result<String, (StatusCode, &'static str)> {
    let Some((md, path)) = report_paths(s, id) else { return Err((StatusCode::NOT_FOUND, "report not ready")) };
    let detailed = std::path::Path::new(&md).with_extension("detailed.md");
    if let Ok(text) = std::fs::read_to_string(detailed) {
        return Ok(text);
    }
    let analysis = std::fs::read(path).ok().and_then(|b| serde_json::from_slice::<GameAnalysis>(&b).ok());
    match analysis {
        Some(a) => Ok(crate::report::render_detailed_markdown(&a)),
        None => Err((StatusCode::NOT_FOUND, "Full JSON is needed to regenerate the detailed report; it may have been cleaned up.")),
    }
}

async fn report_detailed(State(s): State<Shared>, Path(id): Path<u64>) -> Response {
    match detailed_markdown(&s, id) {
        Ok(md) => ([(header::CONTENT_TYPE, "text/markdown; charset=utf-8"), (header::CONTENT_DISPOSITION, "attachment; filename=go-teacher-detailed.md")], md).into_response(),
        Err(error) => error.into_response(),
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
    let mut two_pass = false;
    let mut deep_visits: Option<u64> = None;
    let mut human_profile_target: Option<String> = None;
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
            "human_profile_target" => {
                if let Ok(t) = field.text().await {
                    let t = t.trim().to_string();
                    if !t.is_empty() && t.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') && s.engine_config.read().unwrap().human_model.is_some() {
                        human_profile_target = Some(t);
                    }
                }
            }
            "two_pass" => {
                if let Ok(t) = field.text().await {
                    two_pass = matches!(t.trim(), "1" | "true" | "on" | "yes");
                }
            }
            "deep_visits" => {
                if let Ok(t) = field.text().await {
                    let t = t.trim();
                    if !t.is_empty() {
                        match t.parse::<u64>() {
                            Ok(v) if v > 0 => deep_visits = Some(v),
                            _ => return (StatusCode::BAD_REQUEST, "deep visits must be a positive integer").into_response(),
                        }
                    }
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
        match start_job(&s, name, bytes, max_visits, human_profile.clone(), student, two_pass, deep_visits, human_profile_target.clone()) {
            Ok(id) => ids.push(id),
            Err(e) => return (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
        }
    }
    Json(serde_json::json!({ "job_ids": ids })).into_response()
}

pub fn start_job(s: &Shared, file_name: String, bytes: Vec<u8>, max_visits: Option<u64>, human_profile: Option<String>, student: Option<Color>, two_pass: bool, deep_visits: Option<u64>, human_profile_target: Option<String>) -> Result<u64> {
    if s.engine().is_none() {
        anyhow::bail!("KataGo is not running");
    }
    let text = String::from_utf8_lossy(&bytes).to_string();
    let game = parse_game(&text).map_err(|e| anyhow::anyhow!("{}: {}", file_name, e))?;

    let id = {
        let mut n = s.next_job.lock().unwrap();
        *n += 1;
        *n
    };
    let cancel = CancellationToken::new();
    let n = game.moves.len();
    let job = Job {
        id,
        file_name: file_name.clone(),
        created_at: chrono::Local::now().to_rfc3339(),
        max_visits,
        human_profile: human_profile.clone(),
        human_profile_target,
        student,
        two_pass,
        deep_visits,
        state: JobState::Queued,
        cancel: cancel.clone(),
        markdown: None,
        json: None,
        game: Arc::new(game),
        live: vec![None; n + 1],
        turns_done: 0,
        turns_total: n + 1,
        partial: vec![None; n + 1],
        attempts: 0,
        last_access: Some(std::time::Instant::now()),
        spilled: false,
        phase: String::new(),
        last_updated_turn: None,
    };
    s.jobs.lock().unwrap().insert(id, job);
    spawn_job_task(s.clone(), id);
    Ok(id)
}

/// Wait until the engine is Ready (it may be restarting after a crash). Gives up after `secs`.
async fn wait_for_engine(state: &Shared, secs: u64, cancel: &CancellationToken) -> Option<Arc<Engine>> {
    for _ in 0..secs {
        if cancel.is_cancelled() {
            return None;
        }
        if let Some(e) = state.engine() {
            if e.is_alive() {
                return Some(e);
            }
        }
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }
    None
}

/// Run (or re-run) a job: queue for a slot, analyse, retry once if the engine died mid-way.
fn spawn_job_task(state: Shared, id: u64) {
    tokio::spawn(async move {
        let (cancel, game, opts, file_name) = {
            let mut jobs = state.jobs.lock().unwrap();
            let Some(j) = jobs.get_mut(&id) else { return };
            j.cancel = CancellationToken::new();
            j.state = JobState::Queued;
            (
                j.cancel.clone(),
                j.game.clone(),
                AnalysisOptions {
                    max_visits: j.max_visits,
                    human_profile: j.human_profile.clone(),
                    human_profile_target: j.human_profile_target.clone(),
                    student: j.student,
                    two_pass: j.two_pass,
                    deep_visits: j.deep_visits,
                    ..Default::default()
                },
                j.file_name.clone(),
            )
        };
        let permit = tokio::select! {
            p = state.job_slots.clone().acquire_owned() => p,
            _ = cancel.cancelled() => {
                if let Some(j) = state.jobs.lock().unwrap().get_mut(&id) {
                    j.state = JobState::Cancelled;
                }
                persist_jobs(&state);
                return;
            }
        };
        let Ok(_permit) = permit else { return };

        loop {
            let Some(engine) = wait_for_engine(&state, 240, &cancel).await else {
                let mut jobs = state.jobs.lock().unwrap();
                if let Some(j) = jobs.get_mut(&id) {
                    j.state = if cancel.is_cancelled() { JobState::Cancelled } else { JobState::Failed { error: "KataGo is not running".into() } };
                }
                drop(jobs);
                persist_jobs(&state);
                return;
            };
            let resume = {
                let jobs = state.jobs.lock().unwrap();
                jobs.get(&id).map(|j| j.partial.clone())
            };
            let st = state.clone();
            let progress = move |done: usize, total: usize, turn: Option<&TurnEval>, phase: &str| {
                if let Some(j) = st.jobs.lock().unwrap().get_mut(&id) {
                    j.state = JobState::Running { done, total };
                    j.turns_done = done;
                    j.turns_total = total;
                    j.phase = phase.to_string();
                    if let Some(t) = turn {
                        if let Some(slot) = j.live.get_mut(t.turn) {
                            let mut lt = LiveTurn::from(t);
                            lt.deepened = slot.is_some(); // a second visit to this position is the deep pass
                            *slot = Some(lt);
                        }
                        j.last_updated_turn = Some(t.turn);
                        if let Some(slot) = j.partial.get_mut(t.turn) {
                            if slot.is_none() {
                                *slot = Some(t.clone());
                            }
                        }
                    }
                }
            };
            let result = analyze_game(&engine, (*game).clone(), opts.clone(), cancel.clone(), resume, progress).await;
            let engine_died = !engine.is_alive();
            let retry = {
                let mut jobs = state.jobs.lock().unwrap();
                let Some(job) = jobs.get_mut(&id) else { return };
                match result {
                    Ok(analysis) => {
                        match finish(&state.out_dir, &file_name, &analysis) {
                            Ok((md, json, md_path, json_path)) => {
                                job.markdown = Some(Arc::new(md));
                                job.json = Some(Arc::new(json));
                                job.partial = Vec::new();
                                job.state = JobState::Done { report_path: md_path, json_path, summary: summarize(&analysis) };
                                crate::progress::record(&state.out_dir, &analysis);
                            }
                            Err(e) => job.state = JobState::Failed { error: e.to_string() },
                        }
                        false
                    }
                    Err(e) if cancel.is_cancelled() => {
                        tracing::info!("job {} cancelled: {}", id, e);
                        job.state = JobState::Cancelled;
                        false
                    }
                    Err(e) if engine_died && job.attempts < 1 => {
                        job.attempts += 1;
                        tracing::warn!("job {}: engine died ({}); waiting for restart and resuming", id, e);
                        job.state = JobState::Queued;
                        true
                    }
                    Err(e) => {
                        job.state = JobState::Failed { error: format!("{} (positions kept; use Resume)", e) };
                        false
                    }
                }
            };
            if retry {
                tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                continue;
            }
            persist_jobs(&state);
            return;
        }
    });
}

/// Resume a cancelled or failed job from the positions it already analysed.
async fn resume_job(State(s): State<Shared>, Path(id): Path<u64>) -> Response {
    {
        let mut jobs = s.jobs.lock().unwrap();
        let Some(j) = jobs.get_mut(&id) else { return (StatusCode::NOT_FOUND, "no such job").into_response() };
        if !matches!(j.state, JobState::Cancelled | JobState::Failed { .. }) {
            return (StatusCode::CONFLICT, "only cancelled or failed jobs can be resumed").into_response();
        }
        if j.spilled || j.game.moves.is_empty() && j.turns_total > 1 {
            return (StatusCode::CONFLICT, "this job was restored from disk and cannot be resumed; upload the game again").into_response();
        }
        j.attempts = 0;
        if j.partial.len() != j.game.moves.len() + 1 {
            j.partial = vec![None; j.game.moves.len() + 1];
        }
    }
    if s.engine().is_none() {
        return (StatusCode::SERVICE_UNAVAILABLE, "KataGo is not running").into_response();
    }
    spawn_job_task(s.clone(), id);
    StatusCode::NO_CONTENT.into_response()
}

/// Queue SGF files dropped on the window or opened from Finder, with the saved defaults.
pub fn queue_files(s: &Shared, paths: Vec<PathBuf>) -> Vec<u64> {
    let settings = crate::load_settings();
    let mut ids = Vec::new();
    for p in paths {
        if !p.extension().map_or(false, |e| e.eq_ignore_ascii_case("sgf")) {
            continue;
        }
        let Ok(bytes) = std::fs::read(&p) else { continue };
        let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "game.sgf".into());
        let profile = settings.watch_human_profile.clone().filter(|x| !x.is_empty() && s.engine_config.read().unwrap().human_model.is_some());
        match start_job(s, name, bytes, None, profile, None, true, None, None) {
            Ok(id) => ids.push(id),
            Err(e) => tracing::warn!("cannot queue {}: {}", p.display(), e),
        }
    }
    ids
}

/// Poll the configured watch folder for new SGF files every 10 seconds.
pub async fn watch_folder_task(state: Shared) {
    let seen_path = state.settings_path.with_file_name("watched.json");
    let mut seen: std::collections::HashSet<String> = std::fs::read_to_string(&seen_path)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default();
    let mut first = true;
    loop {
        if state.shutdown.is_cancelled() {
            return;
        }
        let dir = crate::load_settings().watch_dir;
        if let Some(dir) = dir.filter(|d| d.is_dir()) {
            let mut new_files: Vec<PathBuf> = Vec::new();
            if let Ok(rd) = std::fs::read_dir(&dir) {
                for e in rd.flatten() {
                    let p = e.path();
                    if !p.extension().map_or(false, |x| x.eq_ignore_ascii_case("sgf")) {
                        continue;
                    }
                    let key = p.display().to_string();
                    if seen.insert(key) && !first {
                        new_files.push(p);
                    }
                }
            }
            if first {
                // Existing files are not analysed retroactively; only files that appear from now on.
                first = false;
            }
            new_files.sort();
            if !new_files.is_empty() && state.engine().is_some() {
                let ids = queue_files(&state, new_files);
                tracing::info!("watch folder: queued {} game(s)", ids.len());
            }
            if let Ok(t) = serde_json::to_string(&seen) {
                let _ = std::fs::write(&seen_path, t);
            }
        }
        tokio::time::sleep(std::time::Duration::from_secs(10)).await;
    }
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
    std::fs::write(out_dir.join(format!("{}.detailed.md", base)), crate::report::render_detailed_markdown(analysis))?;
    Ok((md, json, md_path, json_path))
}

fn finish(out_dir: &std::path::Path, file_name: &str, analysis: &GameAnalysis) -> Result<(String, String, String, String)> {
    let stamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
    let base = format!("{}_{}", stamp, sanitize(file_name));
    let mut analysis = analysis.clone();
    analysis.history = crate::progress::history_for(out_dir, &analysis);
    if analysis.game.game_name.is_none() {
        analysis.game.game_name = Some(sanitize(file_name));
    }
    let (md, json, md_path, json_path) = write_outputs(out_dir, &base, &analysis)?;
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
