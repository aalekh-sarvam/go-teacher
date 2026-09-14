//! Turns raw KataGo responses into a move-by-move review.

use crate::katago::Engine;
use crate::sgf::{Color, GameRecord};
use anyhow::{anyhow, bail, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisOptions {
    /// `None` means "use maxVisits from the analysis config".
    pub max_visits: Option<u64>,
    /// How many candidate moves to keep per position in the report.
    pub top_moves: usize,
    /// Maximum principal-variation length kept per candidate.
    pub pv_len: usize,
    /// KataGo human-SL profile (e.g. `rank_5k`) for a human policy heat map; needs the human model.
    #[serde(default)]
    pub human_profile: Option<String>,
    /// Which side is the student; `None` = detect from player names.
    #[serde(default)]
    pub student: Option<Color>,
    /// Two-pass mode: a cheap first pass over every position, then key positions re-analysed deeply.
    #[serde(default)]
    pub two_pass: bool,
    /// Visits for the deep second pass (default 1000).
    #[serde(default)]
    pub deep_visits: Option<u64>,
    /// A second, stronger human profile ("what would a player two stones stronger do").
    #[serde(default)]
    pub human_profile_target: Option<String>,
}

impl Default for AnalysisOptions {
    fn default() -> Self {
        AnalysisOptions {
            max_visits: None,
            top_moves: 5,
            pv_len: 10,
            human_profile: None,
            student: None,
            two_pass: false,
            deep_visits: None,
            human_profile_target: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Candidate {
    #[serde(rename = "move")]
    pub mv: String,
    pub visits: u64,
    /// Black's winrate (0..1) if this move is played.
    pub winrate: f64,
    /// Black's expected score lead if this move is played.
    pub score_lead: f64,
    pub prior: f64,
    pub order: usize,
    pub pv: Vec<String>,
    /// KataGo's combined winrate+score utility (Black perspective), used for human-style move choice.
    #[serde(default)]
    pub utility: f64,
    #[serde(default)]
    pub lcb: f64,
    /// Ownership prediction after this move (row-major from the top-left), when requested.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ownership: Option<Vec<f32>>,
}

/// KataGo's evaluation of the position *before* move `turn + 1` is played
/// (turn 0 is the empty/initial board).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnEval {
    pub turn: usize,
    pub to_move: Color,
    /// Black's winrate (0..1).
    pub winrate: f64,
    /// Black's expected score lead in points.
    pub score_lead: f64,
    pub score_stdev: Option<f64>,
    pub visits: u64,
    pub candidates: Vec<Candidate>,
    /// Predicted ownership per point (row-major from the top-left), +1 = Black, -1 = White.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ownership: Option<Vec<f32>>,
    /// Raw network policy per point (row-major from the top-left) plus pass as the last entry; -1 = illegal.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub policy: Option<Vec<f32>>,
    /// Human-style network policy in the same layout, when a human profile was requested.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub human_policy: Option<Vec<f32>>,
    /// Same for the target (stronger) profile.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub human_policy_target: Option<Vec<f32>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Category {
    Best,
    Good,
    Inaccuracy,
    Mistake,
    BigMistake,
    Blunder,
}

impl Category {
    pub fn from_loss(points: f64) -> Category {
        if points < 0.5 {
            Category::Best
        } else if points < 1.5 {
            Category::Good
        } else if points < 3.0 {
            Category::Inaccuracy
        } else if points < 6.0 {
            Category::Mistake
        } else if points < 12.0 {
            Category::BigMistake
        } else {
            Category::Blunder
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Category::Best => "best/excellent",
            Category::Good => "good",
            Category::Inaccuracy => "inaccuracy",
            Category::Mistake => "mistake",
            Category::BigMistake => "big mistake",
            Category::Blunder => "blunder",
        }
    }
    pub fn all() -> [Category; 6] {
        [
            Category::Best,
            Category::Good,
            Category::Inaccuracy,
            Category::Mistake,
            Category::BigMistake,
            Category::Blunder,
        ]
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoveReview {
    /// 1-based move number.
    pub number: usize,
    pub color: Color,
    /// GTP coordinate or "pass".
    pub mv: String,
    pub comment: Option<String>,
    pub winrate_before: f64,
    pub winrate_after: f64,
    pub score_before: f64,
    pub score_after: f64,
    /// Points lost by the player who moved (positive = lost, negative = gained vs. KataGo's estimate).
    pub point_loss: f64,
    /// Winrate lost by the mover, as a fraction (0.05 = 5%).
    pub winrate_loss: f64,
    pub category: Category,
    /// 0-based rank of the played move among KataGo's candidates, if searched at all.
    pub rank: Option<usize>,
    /// KataGo's own evaluation of the played move from the pre-move search, if searched.
    pub played_candidate: Option<Candidate>,
    /// KataGo's preferred move and its alternatives in the position before this move.
    pub alternatives: Vec<Candidate>,
    /// Seconds the player spent on this move, when the record has clock data.
    #[serde(default)]
    pub time_spent: Option<f64>,
    /// Raw-network policy probability of the played move and its 1-based rank over the whole board.
    #[serde(default)]
    pub policy_prob: Option<f32>,
    #[serde(default)]
    pub policy_rank: Option<usize>,
    /// Same for the human-style network, when a profile was requested.
    #[serde(default)]
    pub human_prob: Option<f32>,
    #[serde(default)]
    pub human_rank: Option<usize>,
    #[serde(default)]
    pub human_top: Option<String>,
    #[serde(default)]
    pub human_top_prob: Option<f32>,
    /// Target-profile human policy for the played move and that profile's most common move.
    #[serde(default)]
    pub target_prob: Option<f32>,
    #[serde(default)]
    pub target_rank: Option<usize>,
    #[serde(default)]
    pub target_top: Option<String>,
    #[serde(default)]
    pub target_top_prob: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameAnalysis {
    pub game: GameRecord,
    pub rules: String,
    pub komi: f64,
    pub options: AnalysisOptions,
    pub engine_version: String,
    pub visits_setting: String,
    pub started_at: String,
    pub elapsed_seconds: f64,
    pub turns: Vec<TurnEval>,
    pub reviews: Vec<MoveReview>,
    pub engine_warnings: Vec<String>,
    /// The side being taught and why it was chosen.
    #[serde(default)]
    pub student: Option<Color>,
    #[serde(default)]
    pub student_reason: String,
    #[serde(default)]
    pub teaching: Vec<crate::teaching::TeachingCandidate>,
    #[serde(default)]
    pub praise: Vec<crate::teaching::PraiseCandidate>,
    /// Turns re-analysed at `deep_visits` in two-pass mode.
    #[serde(default)]
    pub deepened: Vec<usize>,
    /// Groups whose life-and-death status changed, per move.
    #[serde(default)]
    pub status_changes: Vec<crate::teaching::StatusChange>,
    /// Per-phase facts for the game-arc narrative.
    #[serde(default)]
    pub phases: Vec<crate::teaching::PhaseFacts>,
    /// Corner opening patterns (13x13 and larger).
    #[serde(default)]
    pub openings: Vec<crate::opening::CornerPattern>,
    /// Earlier games by the same student, for the comparison section (filled in before rendering).
    #[serde(default)]
    pub history: Vec<crate::progress::GameEntry>,
}

/// Map an SGF `RU` string onto a KataGo rules name.
pub fn katago_rules(ru: Option<&str>) -> String {
    let s = ru.unwrap_or("").trim().to_ascii_lowercase();
    match s.as_str() {
        "" => "japanese".to_string(),
        "japanese" | "jp" | "nihon kiin" => "japanese".to_string(),
        "chinese" | "cn" => "chinese".to_string(),
        "korean" | "kr" => "korean".to_string(),
        "aga" => "aga".to_string(),
        "nz" | "new zealand" | "new-zealand" => "new-zealand".to_string(),
        "tromp-taylor" | "tromp taylor" | "tt" => "tromp-taylor".to_string(),
        "bga" => "bga".to_string(),
        "ogs" => "chinese-ogs".to_string(),
        "kgs" => "chinese-kgs".to_string(),
        "goe" | "ing" => "chinese".to_string(),
        other => {
            if other.contains("chinese") {
                "chinese".to_string()
            } else if other.contains("japanese") {
                "japanese".to_string()
            } else if other.contains("korean") {
                "korean".to_string()
            } else if other.contains("aga") {
                "aga".to_string()
            } else {
                "japanese".to_string()
            }
        }
    }
}

pub fn default_komi(rules: &str) -> f64 {
    match rules {
        "japanese" | "korean" => 6.5,
        _ => 7.5,
    }
}

fn move_string(game: &GameRecord, m: &crate::sgf::Move) -> String {
    match m.point {
        Some(p) => p.to_gtp(game.size_y),
        None => "pass".to_string(),
    }
}

/// Optional per-query extras for probe searches.
#[derive(Debug, Clone, Default)]
pub struct QueryExtras {
    /// Restrict both players to these moves (GTP, may include "pass") for `allow_depth` plies.
    pub allow_moves: Option<Vec<String>>,
    pub allow_depth: u32,
    /// Ask for an ownership map per candidate move.
    pub include_moves_ownership: bool,
    /// Human profile for this query (overrides opts.human_profile; None keeps it).
    pub human_profile: Option<String>,
    /// KataGo query priority (higher first).
    pub priority: i32,
    /// Analyse the game record with these extra moves appended (alternating from the side to move).
    pub extra_moves: Vec<(Color, Option<crate::sgf::Coord>)>,
}

/// Query analysing only `turns`, at `max_visits` (None = config default), with probe extras.
pub fn build_query_ext(game: &GameRecord, rules: &str, komi: f64, opts: &AnalysisOptions, turns: &[usize], max_visits: Option<u64>, extras: &QueryExtras) -> Value {
    let mut moves: Vec<Value> = game
        .moves
        .iter()
        .map(|m| serde_json::json!([m.color.letter(), move_string(game, m)]))
        .collect();
    for (color, point) in &extras.extra_moves {
        let mv = point.map(|p| p.to_gtp(game.size_y)).unwrap_or_else(|| "pass".to_string());
        moves.push(serde_json::json!([color.letter(), mv]));
    }
    let mut initial: Vec<Value> = Vec::new();
    for c in &game.setup_black {
        initial.push(serde_json::json!(["B", c.to_gtp(game.size_y)]));
    }
    for c in &game.setup_white {
        initial.push(serde_json::json!(["W", c.to_gtp(game.size_y)]));
    }
    let mut q = serde_json::json!({
        "moves": moves,
        "initialStones": initial,
        "initialPlayer": game.who_moves_first().letter(),
        "rules": rules,
        "komi": komi,
        "boardXSize": game.size_x,
        "boardYSize": game.size_y,
        "analyzeTurns": turns,
        "includePolicy": true,
        "includeOwnership": true,
    });
    if let Some(v) = max_visits {
        q["maxVisits"] = Value::from(v);
    }
    if let Some(p) = extras.human_profile.as_ref().or(opts.human_profile.as_ref()) {
        q["overrideSettings"] = serde_json::json!({ "humanSLProfile": p });
    }
    if extras.include_moves_ownership {
        q["includeMovesOwnership"] = Value::Bool(true);
    }
    if extras.priority != 0 {
        q["priority"] = Value::from(extras.priority);
    }
    if let Some(list) = &extras.allow_moves {
        let depth = extras.allow_depth.max(1);
        q["allowMoves"] = serde_json::json!([
            { "player": "B", "moves": list, "untilDepth": depth },
            { "player": "W", "moves": list, "untilDepth": depth },
        ]);
    }
    q
}

fn f(v: &Value, key: &str) -> Option<f64> {
    v.get(key).and_then(|x| x.as_f64())
}

/// Read a float array, rounded to 3 decimals to keep the JSON small.
fn float_array(v: &Value, key: &str) -> Option<Vec<f32>> {
    let arr = v.get(key)?.as_array()?;
    Some(
        arr.iter()
            .map(|x| ((x.as_f64().unwrap_or(0.0) * 1000.0).round() / 1000.0) as f32)
            .collect(),
    )
}

fn parse_turn(v: &Value, game: &GameRecord, opts: &AnalysisOptions) -> Result<TurnEval> {
    let turn = v
        .get("turnNumber")
        .and_then(|t| t.as_u64())
        .ok_or_else(|| anyhow!("response without turnNumber"))? as usize;
    let root = v.get("rootInfo").ok_or_else(|| anyhow!("response without rootInfo"))?;
    let to_move = match root.get("currentPlayer").and_then(|p| p.as_str()) {
        Some("W") => Color::White,
        Some("B") => Color::Black,
        _ => {
            // Fall back to the game record.
            match game.moves.get(turn) {
                Some(m) => m.color,
                None => game.moves.last().map(|m| m.color.opponent()).unwrap_or(game.who_moves_first()),
            }
        }
    };
    let mut candidates: Vec<Candidate> = v
        .get("moveInfos")
        .and_then(|m| m.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|mi| {
                    Some(Candidate {
                        mv: mi.get("move")?.as_str()?.to_string(),
                        visits: mi.get("visits").and_then(|x| x.as_u64()).unwrap_or(0),
                        winrate: f(mi, "winrate")?,
                        score_lead: f(mi, "scoreLead").or_else(|| f(mi, "scoreMean"))?,
                        prior: f(mi, "prior").unwrap_or(0.0),
                        order: mi.get("order").and_then(|x| x.as_u64()).unwrap_or(0) as usize,
                        utility: f(mi, "utility").unwrap_or(0.0),
                        lcb: f(mi, "lcb").unwrap_or(0.0),
                        ownership: float_array(mi, "ownership"),
                        pv: mi
                            .get("pv")
                            .and_then(|p| p.as_array())
                            .map(|p| {
                                p.iter()
                                    .filter_map(|s| s.as_str().map(|s| s.to_string()))
                                    .take(opts.pv_len)
                                    .collect()
                            })
                            .unwrap_or_default(),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    candidates.sort_by_key(|c| c.order);
    Ok(TurnEval {
        turn,
        to_move,
        winrate: f(root, "winrate").ok_or_else(|| anyhow!("rootInfo without winrate"))?,
        score_lead: f(root, "scoreLead").ok_or_else(|| anyhow!("rootInfo without scoreLead"))?,
        score_stdev: f(root, "scoreStdev"),
        visits: root.get("visits").and_then(|x| x.as_u64()).unwrap_or(0),
        candidates,
        ownership: float_array(v, "ownership"),
        policy: float_array(v, "policy"),
        human_policy: float_array(v, "humanPolicy"),
        human_policy_target: None,
    })
}

/// Index of a GTP move in KataGo's policy array (row-major from the top-left, pass last).
fn policy_index(mv: &str, sx: usize, sy: usize) -> Option<usize> {
    if mv == "pass" {
        return Some(sx * sy);
    }
    crate::sgf::Coord::from_gtp(mv, sy).map(|c| c.y * sx + c.x)
}

/// Probability and 1-based rank of entry `idx` within a policy array.
fn policy_lookup(arr: &[f32], idx: usize) -> Option<(f32, usize)> {
    let p = *arr.get(idx)?;
    if p < 0.0 {
        return None;
    }
    let rank = 1 + arr.iter().filter(|&&x| x > p).count();
    Some((p, rank))
}

fn policy_top(arr: &[f32], sx: usize, sy: usize) -> Option<(String, f32)> {
    let (i, p) = arr.iter().enumerate().max_by(|a, b| a.1.partial_cmp(b.1).unwrap())?;
    let mv = if i == sx * sy {
        "pass".to_string()
    } else {
        crate::sgf::Coord { x: i % sx, y: i / sx }.to_gtp(sy)
    };
    Some((mv, *p))
}

fn build_reviews(game: &GameRecord, turns: &[TurnEval], opts: &AnalysisOptions) -> Vec<MoveReview> {
    let mut out = Vec::with_capacity(game.moves.len());
    for (i, m) in game.moves.iter().enumerate() {
        let before = &turns[i];
        let after = &turns[i + 1];
        let mv = move_string(game, m);
        // Time spent = this player's clock before the move minus after it (byo-yomi resets show as negative and are dropped).
        let time_spent = m.time_left.and_then(|now| {
            let prev = game.moves[..i].iter().rev().find(|p| p.color == m.color).and_then(|p| p.time_left);
            match prev {
                Some(p) if p >= now => Some(p - now),
                Some(_) => None,
                None => None,
            }
        });
        let sign = match m.color {
            Color::Black => 1.0,
            Color::White => -1.0,
        };
        let point_loss = sign * (before.score_lead - after.score_lead);
        let winrate_loss = sign * (before.winrate - after.winrate);
        let rank = before.candidates.iter().position(|c| c.mv == mv);
        let played_candidate = rank.map(|r| before.candidates[r].clone());
        let alternatives: Vec<Candidate> = before.candidates.iter().take(opts.top_moves).cloned().collect();
        let idx = policy_index(&mv, game.size_x, game.size_y);
        let (policy_prob, policy_rank) = match (&before.policy, idx) {
            (Some(arr), Some(i)) => policy_lookup(arr, i).map(|(p, r)| (Some(p), Some(r))).unwrap_or((None, None)),
            _ => (None, None),
        };
        let human_of = |arr: &Option<Vec<f32>>| match (arr, idx) {
            (Some(arr), Some(i)) => {
                let (p, r) = policy_lookup(arr, i).map(|(p, r)| (Some(p), Some(r))).unwrap_or((None, None));
                let (t, tp) = policy_top(arr, game.size_x, game.size_y).map(|(t, tp)| (Some(t), Some(tp))).unwrap_or((None, None));
                (p, r, t, tp)
            }
            _ => (None, None, None, None),
        };
        let (human_prob, human_rank, human_top, human_top_prob) = human_of(&before.human_policy);
        let (target_prob, target_rank, target_top, target_top_prob) = human_of(&before.human_policy_target);
        out.push(MoveReview {
            number: i + 1,
            color: m.color,
            mv,
            comment: m.comment.clone(),
            winrate_before: before.winrate,
            winrate_after: after.winrate,
            score_before: before.score_lead,
            score_after: after.score_lead,
            point_loss,
            winrate_loss,
            category: Category::from_loss(point_loss),
            rank,
            played_candidate,
            alternatives,
            time_spent,
            policy_prob,
            policy_rank,
            human_prob,
            human_rank,
            human_top,
            human_top_prob,
            target_prob,
            target_rank,
            target_top,
            target_top_prob,
        });
    }
    out
}

/// One KataGo query over `turns`; calls `on_turn` as each finished position arrives.
async fn run_query(
    engine: &Engine,
    game: &GameRecord,
    rules: &str,
    komi: f64,
    opts: &AnalysisOptions,
    turns: &[usize],
    max_visits: Option<u64>,
    cancel: &CancellationToken,
    warnings: &mut Vec<String>,
    on_turn: impl FnMut(&TurnEval),
) -> Result<Vec<TurnEval>> {
    run_query_ext(engine, game, rules, komi, opts, turns, max_visits, &QueryExtras::default(), cancel, warnings, on_turn).await
}

/// `run_query` with probe extras (restricted moves, per-move ownership, profile, priority, extra moves).
pub async fn run_query_ext(
    engine: &Engine,
    game: &GameRecord,
    rules: &str,
    komi: f64,
    opts: &AnalysisOptions,
    turns: &[usize],
    max_visits: Option<u64>,
    extras: &QueryExtras,
    cancel: &CancellationToken,
    warnings: &mut Vec<String>,
    mut on_turn: impl FnMut(&TurnEval),
) -> Result<Vec<TurnEval>> {
    if turns.is_empty() {
        return Ok(Vec::new());
    }
    let query = build_query_ext(game, rules, komi, opts, turns, max_visits, extras);
    let (id, mut rx) = engine.query(query).await?;
    let mut out: Vec<TurnEval> = Vec::with_capacity(turns.len());
    let wanted: std::collections::HashSet<usize> = turns.iter().copied().collect();
    let mut seen: std::collections::HashSet<usize> = std::collections::HashSet::new();
    let result: Result<()> = async {
        while seen.len() < wanted.len() {
            let msg = tokio::select! {
                m = rx.recv() => m,
                _ = cancel.cancelled() => {
                    let _ = engine.terminate(&id).await;
                    bail!("analysis cancelled");
                }
            };
            let v = match msg {
                Some(v) => v,
                None => bail!("KataGo stopped responding before the analysis finished"),
            };
            if let Some(err) = v.get("error") {
                let field = v.get("field").and_then(|f| f.as_str()).unwrap_or("");
                bail!("KataGo rejected the game: {} {}", err.as_str().unwrap_or(&err.to_string()), field);
            }
            if let Some(w) = v.get("warning") {
                warnings.push(format!("{} {}", w, v.get("field").and_then(|f| f.as_str()).unwrap_or("")));
                if v.get("rootInfo").is_none() {
                    continue;
                }
            }
            if v.get("isDuringSearch").and_then(|b| b.as_bool()) == Some(true) {
                continue;
            }
            let t = parse_turn(&v, game, opts)?;
            if wanted.contains(&t.turn) && seen.insert(t.turn) {
                on_turn(&t);
                out.push(t);
            }
        }
        Ok(())
    }
    .await;
    engine.unregister(&id);
    result?;
    Ok(out)
}

/// Analyse specific positions of `game` (used for variation exploration). No progress, no cancel.
pub async fn analyze_positions(engine: &Engine, game: &GameRecord, rules: &str, komi: f64, opts: &AnalysisOptions, turns: &[usize]) -> Result<Vec<TurnEval>> {
    let mut warnings = Vec::new();
    run_query(engine, game, rules, komi, opts, turns, opts.max_visits, &CancellationToken::new(), &mut warnings, |_| {}).await
}

/// Positions worth a deep second pass: the student's teaching candidates (before and after),
/// large winrate swings in the undecided part of the game, the opponent's biggest mistakes, the end.
fn deep_turns(reviews: &[MoveReview], teaching: &[crate::teaching::TeachingCandidate], student: Color, n: usize) -> Vec<usize> {
    let mut set: std::collections::BTreeSet<usize> = std::collections::BTreeSet::new();
    for t in teaching {
        set.insert(t.number - 1);
        set.insert(t.number);
    }
    for r in reviews {
        let live = (0.05..=0.95).contains(&r.winrate_before);
        if live && (r.winrate_before - r.winrate_after).abs() >= 0.15 {
            set.insert(r.number - 1);
            set.insert(r.number);
        }
    }
    let mut opp: Vec<&MoveReview> = reviews.iter().filter(|r| r.color != student && r.point_loss >= 2.0).collect();
    opp.sort_by(|a, b| b.point_loss.partial_cmp(&a.point_loss).unwrap());
    for r in opp.iter().take(6) {
        set.insert(r.number - 1);
        set.insert(r.number);
    }
    set.insert(n);
    set.into_iter().filter(|&t| t <= n).collect()
}

/// Run the whole game through KataGo. `progress(done, total, turn)` is called as turns complete;
/// `total` grows when a second pass is scheduled.
pub async fn analyze_game(
    engine: &Engine,
    game: GameRecord,
    opts: AnalysisOptions,
    cancel: CancellationToken,
    resume: Option<Vec<Option<TurnEval>>>,
    mut progress: impl FnMut(usize, usize, Option<&TurnEval>, &str),
) -> Result<GameAnalysis> {
    let rules = katago_rules(game.rules.as_deref());
    let komi = game.komi.unwrap_or_else(|| default_komi(&rules));
    let started = std::time::Instant::now();
    let started_at = chrono::Local::now().to_rfc3339();
    let n = game.moves.len();
    let total1 = n + 1;
    let mut warnings = Vec::new();

    // Pass 1: every position. In two-pass mode this is deliberately cheap.
    let pass1_visits = if opts.two_pass { Some(opts.max_visits.unwrap_or(150)) } else { opts.max_visits };
    // Resume: keep positions already analysed by an interrupted run; analyse only the rest.
    let mut known: Vec<Option<TurnEval>> = match resume {
        Some(v) if v.len() == total1 => v,
        _ => vec![None; total1],
    };
    let missing: Vec<usize> = (0..=n).filter(|&i| known[i].is_none()).collect();
    let mut done = total1 - missing.len();
    let phase1 = if opts.two_pass { "pass 1 of 2: quick analysis of every position" } else { "analysing every position" };
    progress(done, total1, None, phase1);
    let first = run_query(engine, &game, &rules, komi, &opts, &missing, pass1_visits, &cancel, &mut warnings, |t| {
        done += 1;
        progress(done, total1, Some(t), phase1);
    })
    .await?;
    for t in first {
        let i = t.turn;
        known[i] = Some(t);
    }
    let mut turns: Vec<TurnEval> = known.into_iter().map(|t| t.expect("all turns present")).collect();

    let mut reviews = build_reviews(&game, &turns, &opts);
    let (student, student_reason) = crate::teaching::detect_student(&game, opts.student);
    let mut teaching = crate::teaching::select(&game, &turns, &reviews, student, 8);

    // Pass 2: re-analyse the key positions deeply and recompute everything from the merged turns.
    let mut deepened: Vec<usize> = Vec::new();
    if opts.two_pass {
        let deep = deep_turns(&reviews, &teaching, student, n);
        let deep_visits = Some(opts.deep_visits.unwrap_or(1000));
        let total2 = total1 + deep.len();
        let phase2 = format!("pass 2 of 2: deep re-analysis of {} key positions", deep.len());
        progress(done, total2, None, &phase2);
        let second = run_query(engine, &game, &rules, komi, &opts, &deep, deep_visits, &cancel, &mut warnings, |t| {
            done += 1;
            progress(done, total2, Some(t), &phase2);
        })
        .await?;
        for t in second {
            let i = t.turn;
            turns[i] = t;
        }
        deepened = deep;
        reviews = build_reviews(&game, &turns, &opts);
        teaching = crate::teaching::select(&game, &turns, &reviews, student, 8);
    }

    // Target-profile human policy: one network evaluation per position, no search.
    if let Some(target) = opts.human_profile_target.clone().filter(|t| !t.is_empty()) {
        let mut topts = opts.clone();
        topts.human_profile = Some(target);
        let all: Vec<usize> = (0..=n).collect();
        let total3 = total1 + deepened.len() + all.len();
        let phase3 = "target human profile: one network evaluation per position";
        progress(done, total3, None, phase3);
        match run_query(engine, &game, &rules, komi, &topts, &all, Some(1), &cancel, &mut warnings, |_| {
            done += 1;
            progress(done, total3, None, phase3);
        })
        .await
        {
            Ok(extra) => {
                for t in extra {
                    let i = t.turn;
                    turns[i].human_policy_target = t.human_policy;
                }
                reviews = build_reviews(&game, &turns, &opts);
                teaching = crate::teaching::select(&game, &turns, &reviews, student, 8);
            }
            Err(e) => warnings.push(format!("target human profile pass failed: {}", e)),
        }
    }

    let praise = crate::teaching::praise(&reviews, student, 3);
    let status_changes = crate::teaching::status_changes(&game, &turns);
    let openings = crate::opening::corner_patterns(&game, &reviews);
    let phases = crate::teaching::phase_facts(&game, &turns, &reviews, &status_changes, student);
    let visits_setting = if opts.two_pass {
        format!(
            "two-pass: every position at {} visits, then {} key positions at {} visits",
            pass1_visits.unwrap_or(0),
            deepened.len(),
            opts.deep_visits.unwrap_or(1000)
        )
    } else {
        match opts.max_visits {
            Some(v) => format!("{} visits per position (user override)", v),
            None => format!(
                "maxVisits from {} (observed root visits: {})",
                engine.config.config.display(),
                turns.first().map(|t| t.visits).unwrap_or(0)
            ),
        }
    };
    Ok(GameAnalysis {
        game,
        rules,
        komi,
        options: opts,
        engine_version: engine.version.clone(),
        visits_setting,
        started_at,
        elapsed_seconds: started.elapsed().as_secs_f64(),
        turns,
        reviews,
        engine_warnings: warnings,
        student: Some(student),
        student_reason,
        teaching,
        praise,
        deepened,
        status_changes,
        phases,
        openings,
        history: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// End-to-end through the real engine driver against `tests/fake_katago.sh`, which speaks the
    /// analysis protocol with canned numbers: two-pass, target profile, teaching selection, arc
    /// facts, openings and the Markdown renderer, all without a GPU.
    #[tokio::test]
    async fn pipeline_with_fake_engine() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let fake = root.join("tests/fake_katago.sh");
        let tmp = std::env::temp_dir().join(format!("go_teacher_test_{}", std::process::id()));
        std::fs::create_dir_all(&tmp).unwrap();
        let model = tmp.join("model.bin.gz");
        std::fs::write(&model, b"").unwrap();
        let cfg = tmp.join("analysis.cfg");
        std::fs::write(&cfg, "maxVisits = 10\n").unwrap();
        let config = crate::katago::EngineConfig { katago: fake, model: model.clone(), config: cfg, human_model: Some(model.clone()) };
        let engine = crate::katago::Engine::spawn(config, CancellationToken::new()).await.expect("fake engine starts");
        assert_eq!(engine.backend, "Metal");

        let sgf = "(;GM[1]FF[4]SZ[19]KM[6.5]PB[Me]PW[AI (KataGo)];B[pd]BL[600];W[dd]WL[600];B[pq]BL[580];W[dp]WL[590];B[qk]BL[570];W[nc]WL[560];B[qf]BL[500];W[pb]WL[550];B[qc]BL[490];W[kc]WL[540];B[cf]BL[480];W[fc]WL[530];B[bd]BL[470];W[cc]WL[520];B[ci]BL[400];W[qo]WL[510];B[qp]BL[390];W[po]WL[500];B[np]BL[380];W[qm]WL[490])";
        let game = crate::sgf::parse_game(sgf).unwrap();
        let opts = AnalysisOptions {
            max_visits: Some(10),
            two_pass: true,
            deep_visits: Some(20),
            human_profile: Some("rank_10k".into()),
            human_profile_target: Some("rank_3k".into()),
            ..Default::default()
        };
        let mut progress_calls = 0;
        let a = analyze_game(&engine, game, opts, CancellationToken::new(), None, |_, _, _, _| progress_calls += 1).await.expect("analysis");
        assert_eq!(a.turns.len(), 21);
        assert!(progress_calls > 21, "two-pass and target pass report progress");
        assert!(!a.deepened.is_empty());
        assert_eq!(a.student, Some(Color::Black), "White's name looks like an engine");
        assert!(a.reviews.iter().all(|r| r.human_prob.is_some() && r.target_prob.is_some()));
        assert!(a.reviews[2].time_spent.is_some());
        assert!(!a.openings.is_empty(), "19x19 game has corner patterns");
        assert_eq!(a.phases.len(), 3);
        let md = crate::report::render_markdown(&a);
        for needle in ["Report format: 4", "## Teaching candidates", "## Game arc facts", "## Opening patterns", "### Time and loss", "## Appendix"] {
            assert!(md.contains(needle), "report lacks {}", needle);
        }
        let json = serde_json::to_string(&a).unwrap();
        let back: GameAnalysis = serde_json::from_str(&json).unwrap();
        assert_eq!(back.turns.len(), 21);
        engine.shutdown();
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn rules_mapping() {
        assert_eq!(katago_rules(Some("Japanese")), "japanese");
        assert_eq!(katago_rules(Some("Chinese")), "chinese");
        assert_eq!(katago_rules(Some("AGA")), "aga");
        assert_eq!(katago_rules(None), "japanese");
    }

    #[test]
    fn categories() {
        assert_eq!(Category::from_loss(0.1), Category::Best);
        assert_eq!(Category::from_loss(-2.0), Category::Best);
        assert_eq!(Category::from_loss(2.0), Category::Inaccuracy);
        assert_eq!(Category::from_loss(20.0), Category::Blunder);
    }
}
