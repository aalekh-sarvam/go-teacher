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
}

impl Default for AnalysisOptions {
    fn default() -> Self {
        AnalysisOptions {
            max_visits: None,
            top_moves: 5,
            pv_len: 10,
            human_profile: None,
            student: None,
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

pub fn build_query(game: &GameRecord, rules: &str, komi: f64, opts: &AnalysisOptions) -> Value {
    let moves: Vec<Value> = game
        .moves
        .iter()
        .map(|m| serde_json::json!([m.color.letter(), move_string(game, m)]))
        .collect();
    let mut initial: Vec<Value> = Vec::new();
    for c in &game.setup_black {
        initial.push(serde_json::json!(["B", c.to_gtp(game.size_y)]));
    }
    for c in &game.setup_white {
        initial.push(serde_json::json!(["W", c.to_gtp(game.size_y)]));
    }
    let turns: Vec<usize> = (0..=game.moves.len()).collect();
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
    if let Some(v) = opts.max_visits {
        q["maxVisits"] = Value::from(v);
    }
    if let Some(p) = &opts.human_profile {
        q["overrideSettings"] = serde_json::json!({ "humanSLProfile": p });
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
        let (human_prob, human_rank, human_top, human_top_prob) = match (&before.human_policy, idx) {
            (Some(arr), Some(i)) => {
                let (p, r) = policy_lookup(arr, i).map(|(p, r)| (Some(p), Some(r))).unwrap_or((None, None));
                let (t, tp) = policy_top(arr, game.size_x, game.size_y).map(|(t, tp)| (Some(t), Some(tp))).unwrap_or((None, None));
                (p, r, t, tp)
            }
            _ => (None, None, None, None),
        };
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
            policy_prob,
            policy_rank,
            human_prob,
            human_rank,
            human_top,
            human_top_prob,
        });
    }
    out
}

/// Run the whole game through KataGo. `progress(done, total, turn)` is called as turns complete.
pub async fn analyze_game(
    engine: &Engine,
    game: GameRecord,
    opts: AnalysisOptions,
    cancel: CancellationToken,
    mut progress: impl FnMut(usize, usize, Option<&TurnEval>),
) -> Result<GameAnalysis> {
    let rules = katago_rules(game.rules.as_deref());
    let komi = game.komi.unwrap_or_else(|| default_komi(&rules));
    let started = std::time::Instant::now();
    let started_at = chrono::Local::now().to_rfc3339();

    let total = game.moves.len() + 1;
    let query = build_query(&game, &rules, komi, &opts);
    let (id, mut rx) = engine.query(query).await?;

    let mut turns: Vec<Option<TurnEval>> = vec![None; total];
    let mut done = 0usize;
    let mut warnings = Vec::new();
    progress(0, total, None);

    let result: Result<()> = async {
        while done < total {
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
            let t = parse_turn(&v, &game, &opts)?;
            let turn = t.turn;
            if turn < total && turns[turn].is_none() {
                done += 1;
                progress(done, total, Some(&t));
                turns[turn] = Some(t);
            }
        }
        Ok(())
    }
    .await;
    engine.unregister(&id);
    result?;

    let turns: Vec<TurnEval> = turns.into_iter().map(|t| t.expect("all turns present")).collect();
    let reviews = build_reviews(&game, &turns, &opts);
    let (student, student_reason) = crate::teaching::detect_student(&game, opts.student);
    let teaching = crate::teaching::select(&game, &turns, &reviews, student, 8);
    let praise = crate::teaching::praise(&reviews, student, 3);
    let visits_setting = match opts.max_visits {
        Some(v) => format!("{} visits per position (user override)", v),
        None => format!(
            "maxVisits from {} (observed root visits: {})",
            engine.config.config.display(),
            turns.first().map(|t| t.visits).unwrap_or(0)
        ),
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
    })
}

#[cfg(test)]
mod tests {
    use super::*;

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
