//! Cross-game progress: one record per analysed game, kept in `progress.json` in the reports folder.

use crate::analysis::GameAnalysis;
use crate::sgf::Color;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameEntry {
    pub analysed_at: String,
    pub file_stem: String,
    pub date: Option<String>,
    pub student: Option<Color>,
    pub student_name: Option<String>,
    pub opponent_name: Option<String>,
    pub board: String,
    pub result: Option<String>,
    pub moves: usize,
    pub mean_loss: f64,
    pub opening_loss: f64,
    pub middlegame_loss: f64,
    pub endgame_loss: f64,
    pub top1_rate: f64,
    pub mistakes: usize,
    pub blunders: usize,
    pub themes: std::collections::BTreeMap<String, usize>,
    pub final_winrate_black: f64,
    /// This game's rank estimate (numeric scale: 20k = -20 … 1d = 0) from the human-profile ladder.
    #[serde(default)]
    pub rank_estimate: Option<f64>,
}

/// Two entries describe the same game when the record's identity matches (not the analysis time).
pub fn same_game(a: &GameEntry, b: &GameEntry) -> bool {
    a.date == b.date && a.student_name == b.student_name && a.opponent_name == b.opponent_name && a.moves == b.moves && a.result == b.result && a.board == b.board
}

/// Exponentially weighted working rank over the last five distinct games (oldest first) and the
/// current estimate. Returns (value, label, games counted).
pub fn working_rank(history: &[GameEntry], current: Option<f64>) -> Option<(f64, String, usize)> {
    let mut values: Vec<f64> = history.iter().rev().take(5).filter_map(|e| e.rank_estimate).collect();
    values.reverse();
    if let Some(c) = current {
        values.push(c);
    }
    if values.is_empty() {
        return None;
    }
    let mut acc = values[0];
    for v in &values[1..] {
        acc = 0.5 * acc + 0.5 * v;
    }
    Some((acc, crate::probes::rank_label(acc), values.len()))
}

/// The working rank before this game, for choosing the human profiles of the analysis.
pub fn prior_rank(out_dir: &Path, student_name: Option<&str>, student: Color) -> Option<f64> {
    let entries: Vec<GameEntry> = load(out_dir)
        .into_iter()
        .filter(|e| match (student_name, &e.student_name) {
            (Some(x), Some(y)) => x == y,
            _ => e.student == Some(student),
        })
        .collect();
    working_rank(&entries, None).map(|(v, _, _)| v)
}

fn path(out_dir: &Path) -> std::path::PathBuf {
    out_dir.join("progress.json")
}

pub fn load(out_dir: &Path) -> Vec<GameEntry> {
    std::fs::read_to_string(path(out_dir))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

pub fn entry_for(a: &GameAnalysis) -> GameEntry {
    let student = a.student.unwrap_or(Color::Black);
    let mine: Vec<&crate::analysis::MoveReview> = a.reviews.iter().filter(|r| r.color == student).collect();
    let mean = |it: &[&crate::analysis::MoveReview]| if it.is_empty() { 0.0 } else { it.iter().map(|r| r.point_loss.max(0.0)).sum::<f64>() / it.len() as f64 };
    let phase_mean = |name: &str| {
        let v: Vec<&crate::analysis::MoveReview> = mine.iter().copied().filter(|r| crate::teaching::phase_of(r.number, a.game.moves.len()) == name).collect();
        mean(&v)
    };
    let mut themes = std::collections::BTreeMap::new();
    for t in &a.teaching {
        *themes.entry(t.theme.label().to_string()).or_insert(0) += 1;
    }
    let (sname, oname) = match student {
        Color::Black => (a.game.player_black.clone(), a.game.player_white.clone()),
        Color::White => (a.game.player_white.clone(), a.game.player_black.clone()),
    };
    GameEntry {
        analysed_at: a.started_at.clone(),
        file_stem: a.game.game_name.clone().unwrap_or_default(),
        date: a.game.date.clone(),
        student: a.student,
        student_name: sname,
        opponent_name: oname,
        board: format!("{}x{}", a.game.size_x, a.game.size_y),
        result: a.game.result.clone(),
        moves: a.game.moves.len(),
        mean_loss: mean(&mine),
        opening_loss: phase_mean("opening"),
        middlegame_loss: phase_mean("middlegame"),
        endgame_loss: phase_mean("endgame"),
        top1_rate: if mine.is_empty() { 0.0 } else { mine.iter().filter(|r| r.rank == Some(0)).count() as f64 / mine.len() as f64 },
        mistakes: mine.iter().filter(|r| r.point_loss >= 3.0).count(),
        blunders: mine.iter().filter(|r| r.point_loss >= 12.0).count(),
        themes,
        final_winrate_black: a.turns.last().map(|t| t.winrate).unwrap_or(0.5),
        rank_estimate: a.probes.rank_fit.get("estimate").and_then(|e| e.get("rank_value")).and_then(|v| v.as_f64()),
    }
}

/// Append this game's entry (deduplicated by analysed_at) and write the file back.
pub fn record(out_dir: &Path, a: &GameAnalysis) {
    let mut all = load(out_dir);
    let e = entry_for(a);
    // One entry per game: a re-analysis replaces the earlier record of the same game.
    all.retain(|x| x.analysed_at != e.analysed_at && !same_game(x, &e));
    all.push(e);
    all.sort_by(|x, y| x.analysed_at.cmp(&y.analysed_at));
    if let Ok(t) = serde_json::to_string_pretty(&all) {
        let _ = std::fs::create_dir_all(out_dir);
        let _ = std::fs::write(path(out_dir), t);
    }
}

/// Earlier games by the same student (same name when known, else same colour), oldest first.
pub fn history_for(out_dir: &Path, a: &GameAnalysis) -> Vec<GameEntry> {
    let me = entry_for(a);
    load(out_dir)
        .into_iter()
        .filter(|e| e.analysed_at < me.analysed_at && !same_game(e, &me))
        .filter(|e| match (&me.student_name, &e.student_name) {
            (Some(x), Some(y)) => x == y,
            _ => e.student == me.student,
        })
        .collect()
}
