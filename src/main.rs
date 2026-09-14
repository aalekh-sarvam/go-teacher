mod analysis;
mod board;
mod katago;
mod report;
mod report_html;
mod server;
mod sgf;
mod teaching;
mod window;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

/// Engine locations tried in order when no flag / environment variable is given:
/// 1. whatever KaTrain is configured to use (`~/.katrain/config.json`, section `engine`),
/// 2. the engine, network and config bundled inside KaTrain.app (its own default config),
/// 3. a hand-installed KataGo (`brew install katago`) with networks in `~/.katago`.
const BREW_KATAGO: &str = "/opt/homebrew/bin/katago";

/// The analysis config shipped inside the binary (Metal mux settings, 500 visits). Written to the
/// settings directory on first run and used whenever no config is given.
const EMBEDDED_ANALYSIS_CFG: &str = include_str!("../resources/analysis.cfg");

/// Persistent settings: `~/Library/Application Support/GoTeacher/settings.json`.
#[derive(Debug, Default, Clone, serde::Serialize, serde::Deserialize)]
pub struct Settings {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub katago: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub human_model: Option<PathBuf>,
}

pub fn settings_dir() -> PathBuf {
    home().join("Library/Application Support/GoTeacher")
}

pub fn settings_path() -> PathBuf {
    settings_dir().join("settings.json")
}

pub fn load_settings() -> Settings {
    std::fs::read_to_string(settings_path())
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

/// Make sure the embedded analysis config exists on disk and return its path.
fn default_config_path() -> Result<PathBuf> {
    let dir = settings_dir();
    std::fs::create_dir_all(&dir)?;
    let log_dir = dir.join("katago_logs");
    std::fs::create_dir_all(&log_dir)?;
    let path = dir.join("analysis.cfg");
    let rendered = EMBEDDED_ANALYSIS_CFG.replace("{{LOG_DIR}}", &log_dir.display().to_string());
    // Rewrite when missing or when a newer build changed the embedded config.
    if std::fs::read_to_string(&path).map(|cur| cur != rendered).unwrap_or(true) {
        std::fs::write(&path, rendered)?;
    }
    Ok(path)
}

/// Analyze Go games (SGF) move by move with KataGo and write a teaching report.
#[derive(Parser, Debug)]
#[command(name = "go_teacher", version, about)]
struct Cli {
    /// Path to the KataGo executable (default: /opt/homebrew/bin/katago, else KaTrain's bundled engine).
    #[arg(long, env = "GO_TEACHER_KATAGO")]
    katago: Option<PathBuf>,
    /// Path to the KataGo neural-network model (default: ~/.katago/default_model.bin.gz, else KaTrain's).
    #[arg(long, env = "GO_TEACHER_MODEL")]
    model: Option<PathBuf>,
    /// Path to the KataGo analysis config (default: ~/.katago/default_analysis.cfg, else KaTrain's).
    #[arg(long, env = "GO_TEACHER_CONFIG")]
    config: Option<PathBuf>,
    /// Path to KataGo's human-style network, used for human policy heat maps
    /// (default: ~/.katago/default_human_model.bin.gz if it exists).
    #[arg(long, env = "GO_TEACHER_HUMAN_MODEL")]
    human_model: Option<PathBuf>,
    /// Do not load the human-style network (faster start, less GPU memory).
    #[arg(long)]
    no_human_model: bool,
    /// Directory where reports are written. Defaults to ./reports on the command line and to
    /// ~/Documents/GoTeacher when running as a macOS app.
    #[arg(long, env = "GO_TEACHER_OUT_DIR")]
    out_dir: Option<PathBuf>,
    /// Append logs to this file instead of stderr (the app bundle uses ~/Library/Logs/GoTeacher.log).
    #[arg(long, env = "GO_TEACHER_LOG_FILE")]
    log_file: Option<PathBuf>,
    /// TCP port for the web UI (127.0.0.1 only).
    #[arg(long, default_value_t = 8642, env = "GO_TEACHER_PORT")]
    port: u16,
    /// Do not open the browser automatically (only meaningful with --browser).
    #[arg(long)]
    no_open: bool,
    /// Show the UI in your web browser instead of the app window.
    #[arg(long, env = "GO_TEACHER_BROWSER")]
    browser: bool,
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Start the local web UI (default when no subcommand is given).
    Serve,
    /// Analyze one or more SGF files from the command line, without the web UI.
    Analyze {
        /// SGF files to analyze.
        #[arg(required = true)]
        files: Vec<PathBuf>,
        /// Override maxVisits from the analysis config.
        #[arg(long)]
        visits: Option<u64>,
        /// Human-style policy profile to record per position (e.g. rank_5k); needs the human model.
        #[arg(long)]
        human_profile: Option<String>,
        /// Which side is the student (B or W); default: detect from player names.
        #[arg(long)]
        student: Option<String>,
    },
}

fn existing(p: PathBuf) -> Option<PathBuf> {
    if p.exists() { Some(p) } else { None }
}

/// KaTrain.app's resource directory, if KaTrain is installed.
fn katrain_resources() -> Option<PathBuf> {
    ["/Applications/KaTrain.app", &format!("{}/Applications/KaTrain.app", home().display())]
        .iter()
        .map(|a| PathBuf::from(a).join("Contents/Resources"))
        .find(|p| p.join("katrain").is_dir())
}

#[derive(Default, Debug)]
struct KatrainEngine {
    katago: Option<PathBuf>,
    model: Option<PathBuf>,
    config: Option<PathBuf>,
    human_model: Option<PathBuf>,
    source: &'static str,
}

/// Read the `engine` section of a KaTrain config.json. Relative paths such as
/// `katrain/models/x.bin.gz` are inside KaTrain.app's resources; an empty `katago` means the
/// bundled executable.
fn read_katrain_config(path: &std::path::Path, res: Option<&PathBuf>) -> Option<KatrainEngine> {
    let text = std::fs::read_to_string(path).ok()?;
    let v: serde_json::Value = serde_json::from_str(&text).ok()?;
    let eng = v.get("engine")?;
    let resolve = |key: &str| -> Option<PathBuf> {
        let raw = eng.get(key)?.as_str()?.trim();
        if raw.is_empty() {
            return None;
        }
        let raw = raw.replace('~', &home().display().to_string());
        let p = PathBuf::from(&raw);
        if p.is_absolute() {
            return existing(p);
        }
        existing(res?.join(&raw))
    };
    let bundled_bin = res.map(|r| r.join("katrain/KataGo/katago-osx")).and_then(existing);
    Some(KatrainEngine {
        katago: resolve("katago").or(bundled_bin),
        model: resolve("model"),
        config: resolve("config"),
        human_model: resolve("humanlike_model"),
        source: "",
    })
}

/// KaTrain's engine: the user's KaTrain settings first, then KaTrain's shipped defaults.
fn katrain_engine() -> Option<KatrainEngine> {
    let res = katrain_resources();
    let mut user = read_katrain_config(&home().join(".katrain/config.json"), res.as_ref()).unwrap_or_default();
    let bundled = res
        .as_ref()
        .and_then(|r| read_katrain_config(&r.join("katrain/config.json"), Some(r)))
        .unwrap_or_default();
    let from_user = user.katago.is_some() || user.model.is_some() || user.config.is_some();
    user.katago = user.katago.or(bundled.katago);
    user.model = user.model.or(bundled.model);
    user.config = user.config.or(bundled.config);
    user.human_model = user.human_model.or(bundled.human_model);
    if user.katago.is_none() && user.model.is_none() && user.config.is_none() {
        return None;
    }
    user.source = if from_user { "the engine configured in KaTrain (~/.katrain/config.json)" } else { "KaTrain's bundled KataGo" };
    Some(user)
}

/// Engine paths given on the command line / environment (kept so the engine can be re-resolved
/// after the user edits the settings without relaunching).
#[derive(Debug, Clone, Default)]
pub struct EnginePaths {
    pub katago: Option<PathBuf>,
    pub model: Option<PathBuf>,
    pub config: Option<PathBuf>,
    pub human_model: Option<PathBuf>,
    pub no_human_model: bool,
}

impl EnginePaths {
    fn from_cli(cli: &Cli) -> EnginePaths {
        EnginePaths {
            katago: cli.katago.clone(),
            model: cli.model.clone(),
            config: cli.config.clone(),
            human_model: cli.human_model.clone(),
            no_human_model: cli.no_human_model,
        }
    }
}

/// Pick engine files. For each file, in order: flag / environment variable, the settings file,
/// then the fallbacks: executable and network from KaTrain (its settings, then its bundle) or a
/// Homebrew KataGo with `~/.katago`; the analysis config always defaults to the embedded one.
pub fn resolve_engine(cli: &EnginePaths) -> (katago::EngineConfig, String) {
    let own = home().join(".katago");
    let saved = load_settings();
    let kt = katrain_engine().unwrap_or_default();
    let mut sources: Vec<String> = Vec::new();
    let kt_label = if kt.source.contains("bundled") { "KaTrain bundle" } else { "KaTrain settings" };

    let pick = |flag: &Option<PathBuf>, saved_p: &Option<PathBuf>, fallbacks: &[Option<PathBuf>], what: &str, sources: &mut Vec<String>| -> Option<PathBuf> {
        if let Some(p) = flag {
            sources.push(format!("{}: flag/env", what));
            return Some(p.clone());
        }
        if let Some(p) = saved_p {
            sources.push(format!("{}: settings.json", what));
            return Some(p.clone());
        }
        for (i, f) in fallbacks.iter().enumerate() {
            if let Some(p) = f.clone().and_then(existing) {
                sources.push(format!("{}: {}", what, [kt_label, "KaTrain bundle", "~/.katago"][i.min(2)]));
                return Some(p);
            }
        }
        None
    };
    let res = katrain_resources();
    let katago_bin = pick(
        &cli.katago,
        &saved.katago,
        &[kt.katago.clone(), res.as_ref().map(|r| r.join("katrain/KataGo/katago-osx")), Some(PathBuf::from(BREW_KATAGO))],
        "engine",
        &mut sources,
    )
    .unwrap_or_else(|| PathBuf::from(BREW_KATAGO));
    let model = pick(&cli.model, &saved.model, &[kt.model.clone(), None, Some(own.join("default_model.bin.gz"))], "network", &mut sources)
        .unwrap_or_else(|| own.join("default_model.bin.gz"));
    let config = match pick(&cli.config, &saved.config, &[], "config", &mut sources) {
        Some(p) => p,
        None => {
            sources.push("config: built-in default".to_string());
            default_config_path().unwrap_or_else(|_| own.join("default_analysis.cfg"))
        }
    };
    let human_model = if cli.no_human_model {
        None
    } else {
        pick(&cli.human_model, &saved.human_model, &[kt.human_model.clone(), None, Some(own.join("default_human_model.bin.gz"))], "human network", &mut sources)
    };
    (katago::EngineConfig { katago: katago_bin, model, config, human_model }, sources.join(", "))
}

/// True when the executable lives inside a `.app` bundle (launched from Finder / Dock).
fn running_as_app() -> bool {
    std::env::current_exe()
        .map(|p| p.to_string_lossy().contains(".app/Contents/MacOS/"))
        .unwrap_or(false)
}

fn home() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/tmp"))
}

fn init_logging(log_file: Option<&PathBuf>) -> Result<()> {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info,katago=warn"));
    match log_file {
        Some(path) => {
            if let Some(dir) = path.parent() {
                std::fs::create_dir_all(dir)?;
            }
            let file = std::fs::OpenOptions::new().create(true).append(true).open(path)
                .with_context(|| format!("cannot open log file {}", path.display()))?;
            tracing_subscriber::fmt()
                .with_env_filter(filter)
                .with_target(false)
                .with_ansi(false)
                .with_writer(std::sync::Mutex::new(file))
                .init();
        }
        None => {
            tracing_subscriber::fmt().with_env_filter(filter).with_target(false).init();
        }
    }
    Ok(())
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let as_app = running_as_app();

    let log_file = cli.log_file.clone().or_else(|| {
        if as_app {
            Some(home().join("Library/Logs/GoTeacher.log"))
        } else {
            None
        }
    });
    init_logging(log_file.as_ref())?;
    if as_app {
        tracing::info!("launched as an app bundle");
    }
    let paths = EnginePaths::from_cli(&cli);
    let (engine_cfg, source) = resolve_engine(&paths);
    eprintln!("Engine: {}", source);
    tracing::info!("engine: {} ({} / {} / {})", source, engine_cfg.katago.display(), engine_cfg.model.display(), engine_cfg.config.display());
    let out_dir = cli.out_dir.clone().unwrap_or_else(|| {
        if as_app {
            home().join("Documents/GoTeacher")
        } else {
            PathBuf::from("reports")
        }
    });
    let out_dir = std::path::absolute(&out_dir).unwrap_or(out_dir);

    let runtime = tokio::runtime::Runtime::new()?;
    match cli.command.unwrap_or(Command::Serve) {
        Command::Serve => serve(runtime, paths, out_dir, cli.port, cli.no_open, !cli.browser),
        Command::Analyze { files, visits, human_profile, student } => {
            let student = match student.as_deref().map(|s| s.trim().to_ascii_uppercase()) {
                None => None,
                Some(s) if s == "B" || s == "BLACK" => Some(sgf::Color::Black),
                Some(s) if s == "W" || s == "WHITE" => Some(sgf::Color::White),
                Some(s) => anyhow::bail!("--student must be B or W, not {:?}", s),
            };
            runtime.block_on(analyze_files(engine_cfg, out_dir, files, visits, human_profile, student))
        }
    }
}

/// macOS notification + sound once the engine has loaded (there is no terminal to watch).
fn notify_ready() {
    let _ = std::process::Command::new("osascript")
        .arg("-e")
        .arg("display notification \"KataGo has loaded. You can upload games now.\" with title \"Go Teacher\" subtitle \"Engine ready\"")
        .spawn();
    let _ = std::process::Command::new("afplay").arg("/System/Library/Sounds/Glass.aiff").spawn();
}

fn open_browser(url: &str) {
    let _ = std::process::Command::new("open").arg(url).spawn();
}

/// Show a native dialog when there is no terminal to print to.
fn alert(title: &str, message: &str) {
    if !running_as_app() {
        return;
    }
    let script = format!(
        "display alert \"{}\" message \"{}\" as critical",
        title.replace('"', "'"),
        message.replace('"', "'").replace('\\', "/")
    );
    let _ = std::process::Command::new("osascript").arg("-e").arg(script).status();
}

/// (Re)start KataGo in the background using the current settings. Safe to call again after the
/// user changed the engine paths; a running engine is stopped first.
pub fn start_engine(state: Arc<server::AppState>, handle: tokio::runtime::Handle, notify: bool) {
    let (engine_cfg, source) = resolve_engine(&state.cli_paths);
    if let Some(old) = state.engine() {
        old.shutdown();
    }
    *state.engine_config.write().unwrap() = engine_cfg.clone();
    *state.engine_source.write().unwrap() = source.clone();
    *state.engine.write().unwrap() = server::EngineState::Starting;
    let cancel = state.shutdown.clone();
    eprintln!("Engine: {}", source);
    eprintln!("Starting KataGo ({}) — loading the model can take a little while...", engine_cfg.katago.display());
    tracing::info!("starting KataGo: {} ({} / {} / {})", source, engine_cfg.katago.display(), engine_cfg.model.display(), engine_cfg.config.display());
    handle.spawn(async move {
        let result = match engine_cfg.validate() {
            Ok(()) => katago::Engine::spawn(engine_cfg, cancel.clone()).await,
            Err(e) => Err(e),
        };
        if cancel.is_cancelled() {
            if let Ok(engine) = &result {
                engine.shutdown();
            }
            return;
        }
        let mut slot = state.engine.write().unwrap();
        match result {
            Ok(engine) => {
                eprintln!("KataGo ready");
                *slot = server::EngineState::Ready(engine);
                if notify {
                    notify_ready();
                }
            }
            Err(e) => {
                tracing::error!("KataGo failed to start: {}", e);
                eprintln!("KataGo failed to start: {}", e);
                *slot = server::EngineState::Failed(e.to_string());
            }
        }
    });
}

fn serve(runtime: tokio::runtime::Runtime, paths: EnginePaths, out_dir: PathBuf, port: u16, no_open: bool, windowed: bool) -> Result<()> {
    let addr = format!("127.0.0.1:{}", port);
    let url = format!("http://{}", addr);

    // Bind first: if another instance already owns the port, just show its page and exit.
    let listener = match runtime.block_on(tokio::net::TcpListener::bind(&addr)) {
        Ok(l) => l,
        Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
            let already_running = runtime.block_on(reqwest_free_probe(&url));
            if already_running {
                tracing::info!("another go_teacher instance is already serving {}; showing it", url);
                eprintln!("go_teacher is already running at {}", url);
                if windowed {
                    // A window onto the other instance; closing it leaves that instance running.
                    window::run(url, tokio_util::sync::CancellationToken::new(), runtime.handle().clone(), || {});
                }
                if !no_open {
                    open_browser(&url);
                }
                return Ok(());
            }
            let msg = format!("Port {} is in use by another program. Start go_teacher with --port <other>.", port);
            alert("Go Teacher cannot start", &msg);
            anyhow::bail!(msg);
        }
        Err(e) => return Err(e).with_context(|| format!("cannot listen on {}", addr)),
    };

    let shutdown = tokio_util::sync::CancellationToken::new();
    let (engine_cfg, source) = resolve_engine(&paths);
    let state = Arc::new(server::AppState {
        engine: std::sync::RwLock::new(server::EngineState::Starting),
        engine_config: std::sync::RwLock::new(engine_cfg),
        engine_source: std::sync::RwLock::new(source),
        cli_paths: paths,
        settings_path: settings_path(),
        katrain_installed: katrain_resources().is_some(),
        windowed,
        out_dir: out_dir.clone(),
        jobs: Mutex::new(Default::default()),
        next_job: Mutex::new(0),
        shutdown: shutdown.clone(),
    });

    // Serve the UI immediately; the page shows "starting" until KataGo is ready.
    let app = server::router(state.clone());
    {
        let shutdown = shutdown.clone();
        runtime.spawn(async move {
            let result = axum::serve(listener, app)
                .with_graceful_shutdown(async move { shutdown.cancelled().await })
                .await;
            if let Err(e) = result {
                tracing::error!("server error: {}", e);
            }
        });
    }

    // Load KataGo in the background; the page shows the setup guide if nothing is found.
    start_engine(state.clone(), runtime.handle().clone(), windowed || running_as_app());

    eprintln!("go_teacher UI: {}", url);
    eprintln!("Reports are written to {}", out_dir.display());
    tracing::info!("ui at {}; reports in {}", url, out_dir.display());

    // Ctrl-C also quits.
    {
        let shutdown = shutdown.clone();
        runtime.spawn(async move {
            let _ = tokio::signal::ctrl_c().await;
            shutdown.cancel();
        });
    }

    let stop_engine = {
        let state = state.clone();
        move || {
            tracing::info!("shutting down");
            if let Some(engine) = state.engine() {
                engine.shutdown();
            }
        }
    };

    if windowed {
        window::run(url, shutdown, runtime.handle().clone(), stop_engine)
    } else {
        if !no_open {
            open_browser(&url);
        }
        runtime.block_on(shutdown.cancelled());
        stop_engine();
        Ok(())
    }
}

/// Minimal HTTP probe (no extra dependency): is a go_teacher instance answering on `url`?
async fn reqwest_free_probe(url: &str) -> bool {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let host = url.trim_start_matches("http://");
    let Ok(mut stream) = tokio::net::TcpStream::connect(host).await else { return false };
    let req = format!("GET /api/status HTTP/1.0\r\nHost: {}\r\n\r\n", host);
    if stream.write_all(req.as_bytes()).await.is_err() {
        return false;
    }
    let mut buf = Vec::new();
    let _ = tokio::time::timeout(std::time::Duration::from_secs(2), stream.read_to_end(&mut buf)).await;
    String::from_utf8_lossy(&buf).contains("engine_version")
}

async fn analyze_files(engine_cfg: katago::EngineConfig, out_dir: PathBuf, files: Vec<PathBuf>, visits: Option<u64>, human_profile: Option<String>, student: Option<sgf::Color>) -> Result<()> {
    engine_cfg.validate()?;
    // Parse everything first so a bad file fails before the model loads.
    let mut games = Vec::new();
    for f in &files {
        let bytes = std::fs::read(f).with_context(|| format!("cannot read {}", f.display()))?;
        let game = sgf::parse_game(&String::from_utf8_lossy(&bytes)).with_context(|| format!("cannot parse {}", f.display()))?;
        games.push((f.clone(), game));
    }
    eprintln!("Starting KataGo...");
    let engine = katago::Engine::spawn(engine_cfg, tokio_util::sync::CancellationToken::new()).await?;
    for (path, game) in games {
        let name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("game").to_string();
        eprintln!("Analyzing {} ({} moves)", path.display(), game.moves.len());
        let opts = analysis::AnalysisOptions {
            max_visits: visits,
            human_profile: human_profile.clone(),
            student,
            ..Default::default()
        };
        let cancel = tokio_util::sync::CancellationToken::new();
        let analysis = analysis::analyze_game(&engine, game, opts, cancel, |done, total, _| {
            if done % 10 == 0 || done == total {
                eprintln!("  {}/{} positions", done, total);
            }
        })
        .await?;
        let stamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
        let base = format!("{}_{}", stamp, name);
        let (_, _, md_path, json_path) = server::write_outputs(&out_dir, &base, &analysis)?;
        eprintln!("  wrote {}\n  wrote {}", md_path.display(), json_path.display());
    }
    Ok(())
}
