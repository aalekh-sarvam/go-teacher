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

const DEFAULT_KATAGO: &str = "/opt/homebrew/bin/katago";
const DEFAULT_MODEL: &str = "/Users/aalekhsharan/.katago/default_model.bin.gz";
const DEFAULT_CONFIG: &str = "/Users/aalekhsharan/.katago/default_analysis.cfg";
const DEFAULT_HUMAN_MODEL: &str = "/Users/aalekhsharan/.katago/default_human_model.bin.gz";

/// Analyze Go games (SGF) move by move with KataGo and write a teaching report.
#[derive(Parser, Debug)]
#[command(name = "go_teacher", version, about)]
struct Cli {
    /// Path to the KataGo executable.
    #[arg(long, default_value = DEFAULT_KATAGO, env = "GO_TEACHER_KATAGO")]
    katago: PathBuf,
    /// Path to the KataGo neural-network model.
    #[arg(long, default_value = DEFAULT_MODEL, env = "GO_TEACHER_MODEL")]
    model: PathBuf,
    /// Path to the KataGo analysis config.
    #[arg(long, default_value = DEFAULT_CONFIG, env = "GO_TEACHER_CONFIG")]
    config: PathBuf,
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
    let human_model = if cli.no_human_model {
        None
    } else {
        cli.human_model.clone().or_else(|| {
            let p = PathBuf::from(DEFAULT_HUMAN_MODEL);
            if p.exists() {
                Some(p)
            } else {
                None
            }
        })
    };
    let engine_cfg = katago::EngineConfig {
        katago: cli.katago.clone(),
        model: cli.model.clone(),
        config: cli.config.clone(),
        human_model,
    };
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
        Command::Serve => serve(runtime, engine_cfg, out_dir, cli.port, cli.no_open, !cli.browser),
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

fn serve(runtime: tokio::runtime::Runtime, engine_cfg: katago::EngineConfig, out_dir: PathBuf, port: u16, no_open: bool, windowed: bool) -> Result<()> {
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
    let state = Arc::new(server::AppState {
        engine: std::sync::RwLock::new(server::EngineState::Starting),
        engine_config: engine_cfg.clone(),
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

    // Load KataGo in the background.
    {
        let state = state.clone();
        let cancel = shutdown.clone();
        let notify = windowed || running_as_app();
        eprintln!("Starting KataGo ({}) — loading the model can take a little while...", engine_cfg.katago.display());
        tracing::info!("starting KataGo at {}", engine_cfg.katago.display());
        runtime.spawn(async move {
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
