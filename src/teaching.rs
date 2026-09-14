//! Derived teaching signals: who the student is, which mistakes are most instructive, and
//! board facts (captures, atari, where the points went) that a language model cannot infer.

use crate::analysis::{Category, MoveReview, TurnEval};
use crate::board::Board;
use crate::sgf::{Color, Coord, GameRecord};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeachingCandidate {
    pub number: usize,
    pub color: Color,
    pub mv: String,
    pub best: Option<String>,
    pub point_loss: f64,
    pub category: Category,
    pub winrate_before: f64,
    /// Winrate before the move was already below 5% or above 95%.
    pub decided_before: bool,
    pub opponent_last: Option<String>,
    /// Chebyshev distance between the played move and KataGo's best move.
    pub distance_to_best: Option<usize>,
    pub best_answers_last: bool,
    pub played_answers_last: bool,
    /// Move number at which the played stone was captured, if within 10 moves.
    pub captured_at: Option<usize>,
    pub loss_region: Option<String>,
    pub loss_region_points: f64,
    /// Groups in atari after the move, e.g. "Black D4 (3 stones)".
    pub atari_after: Vec<String>,
    pub policy_prob: Option<f32>,
    pub policy_rank: Option<usize>,
    pub human_prob: Option<f32>,
    pub human_rank: Option<usize>,
    pub human_top: Option<String>,
    pub human_top_prob: Option<f32>,
    /// Position before the move, for building puzzles.
    pub black_stones: Vec<String>,
    pub white_stones: Vec<String>,
    /// Plain-language facts derived from the numbers above.
    pub hints: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PraiseCandidate {
    pub number: usize,
    pub color: Color,
    pub mv: String,
    pub second: String,
    /// How many points worse KataGo's second choice was.
    pub gap: f64,
    pub winrate_before: f64,
}

fn looks_like_engine(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.starts_with("ai (")
        || n.starts_with("ai:")
        || n == "ai"
        || n.contains("katago")
        || n.contains("leela")
        || n.contains("gnugo")
        || n.contains("gnu go")
        || n.contains("pachi")
        || n.contains("fuego")
        || n.split(|c: char| !c.is_alphanumeric()).any(|w| w == "bot" || w == "ai")
}

/// Which side is the student: an explicit override, else the side whose opponent looks like an engine, else Black.
pub fn detect_student(game: &GameRecord, override_: Option<Color>) -> (Color, String) {
    if let Some(c) = override_ {
        return (c, "set by the user".to_string());
    }
    let b = game.player_black.as_deref().unwrap_or("");
    let w = game.player_white.as_deref().unwrap_or("");
    match (looks_like_engine(b), looks_like_engine(w)) {
        (true, false) => (Color::White, format!("Black's name \"{}\" looks like an engine", b)),
        (false, true) => (Color::Black, format!("White's name \"{}\" looks like an engine", w)),
        _ => (Color::Black, "default (no engine name detected); pass --student W to override".to_string()),
    }
}

/// Board after k moves, for k = 0..=n.
pub fn boards(game: &GameRecord) -> Vec<Board> {
    let mut b = Board::new(game.size_x, game.size_y);
    for c in &game.setup_black {
        b.set(*c, Some(Color::Black));
    }
    for c in &game.setup_white {
        b.set(*c, Some(Color::White));
    }
    let mut out = Vec::with_capacity(game.moves.len() + 1);
    out.push(b.clone());
    for m in &game.moves {
        if let Some(p) = m.point {
            b.play(m.color, p);
        }
        out.push(b.clone());
    }
    out
}

pub fn region_name(c: Coord, sx: usize, sy: usize) -> &'static str {
    let rx = (c.x * 3 / sx).min(2);
    let ry = (c.y * 3 / sy).min(2);
    const NAMES: [[&str; 3]; 3] = [
        ["upper left", "top side", "upper right"],
        ["left side", "centre", "right side"],
        ["lower left", "bottom side", "lower right"],
    ];
    NAMES[ry][rx]
}

fn chebyshev(a: Coord, b: Coord) -> usize {
    a.x.abs_diff(b.x).max(a.y.abs_diff(b.y))
}

fn gtp(mv: &str, sy: usize) -> Option<Coord> {
    Coord::from_gtp(mv, sy)
}

/// Region where the mover lost the most ownership between two positions, in points.
fn loss_region(before: &TurnEval, after: &TurnEval, mover: Color, sx: usize, sy: usize) -> Option<(String, f64)> {
    let (ob, oa) = (before.ownership.as_ref()?, after.ownership.as_ref()?);
    if ob.len() != sx * sy || oa.len() != sx * sy {
        return None;
    }
    let sign = if mover == Color::Black { 1.0 } else { -1.0 };
    let mut sums: std::collections::HashMap<&str, f64> = std::collections::HashMap::new();
    for y in 0..sy {
        for x in 0..sx {
            let i = y * sx + x;
            let d = sign * (oa[i] as f64 - ob[i] as f64);
            *sums.entry(region_name(Coord { x, y }, sx, sy)).or_insert(0.0) += d;
        }
    }
    let (name, val) = sums.into_iter().min_by(|a, b| a.1.partial_cmp(&b.1).unwrap())?;
    if val <= -1.0 {
        Some((name.to_string(), -val))
    } else {
        None
    }
}

fn atari_list(board: &Board, sy: usize) -> Vec<String> {
    let mut out = Vec::new();
    for (color, mut group, libs) in board.groups() {
        if libs == 1 {
            group.sort_by_key(|c| (c.y, c.x));
            out.push(format!("{} {} ({} stone{})", color.name(), group[0].to_gtp(sy), group.len(), if group.len() == 1 { "" } else { "s" }));
        }
    }
    out
}

fn stones(board: &Board, color: Color, sy: usize) -> Vec<String> {
    let mut v = board.stones(color);
    v.sort_by_key(|c| (c.y, c.x));
    v.iter().map(|c| c.to_gtp(sy)).collect()
}

fn build(i: usize, game: &GameRecord, turns: &[TurnEval], reviews: &[MoveReview], boards: &[Board]) -> TeachingCandidate {
    let r = &reviews[i];
    let (sx, sy) = (game.size_x, game.size_y);
    let played = gtp(&r.mv, sy);
    let best = r.alternatives.first().map(|c| c.mv.clone());
    let best_c = best.as_deref().and_then(|b| gtp(b, sy));
    let opponent_last = if i > 0 { Some(reviews[i - 1].mv.clone()) } else { None };
    let last_c = opponent_last.as_deref().and_then(|m| gtp(m, sy));
    let distance_to_best = match (played, best_c) {
        (Some(p), Some(b)) => Some(chebyshev(p, b)),
        _ => None,
    };
    let near = |a: Option<Coord>, b: Option<Coord>| matches!((a, b), (Some(a), Some(b)) if chebyshev(a, b) <= 2);
    let best_answers_last = near(best_c, last_c);
    let played_answers_last = near(played, last_c);

    let captured_at = played.and_then(|p| {
        (i + 2..=(i + 10).min(game.moves.len())).find(|&k| boards[k].get(p) != Some(r.color))
    });
    let region = loss_region(&turns[i], &turns[i + 1], r.color, sx, sy);
    let atari_after = atari_list(&boards[i + 1], sy);
    let decided_before = !(0.05..=0.95).contains(&r.winrate_before);

    let mut hints = Vec::new();
    if decided_before {
        hints.push(format!(
            "The game was already decided before this move (Black winrate {:.0}%); judge it by the point loss, not the winrate swing.",
            r.winrate_before * 100.0
        ));
    }
    if let Some(k) = captured_at {
        hints.push(format!("The stone played at {} was captured at move {}.", r.mv, k));
    }
    if best_answers_last && !played_answers_last {
        hints.push(format!(
            "KataGo's move {} answers the opponent's last move at {}; the played move went elsewhere (a tenuki while threatened).",
            best.as_deref().unwrap_or("?"),
            opponent_last.as_deref().unwrap_or("?")
        ));
    } else if played_answers_last && !best_answers_last {
        hints.push(format!(
            "The played move responds locally to {}, but KataGo prefers to play elsewhere at {} (the local move was not urgent).",
            opponent_last.as_deref().unwrap_or("?"),
            best.as_deref().unwrap_or("?")
        ));
    }
    match distance_to_best {
        Some(d) if d <= 2 => hints.push("The better move is in the same area: a shape or reading problem rather than direction.".to_string()),
        Some(d) if d >= 5 => hints.push(format!("The better move is {} points away: a direction or priority problem.", d)),
        _ => {}
    }
    if let Some((name, pts)) = &region {
        hints.push(format!("About {:.0} points changed hands in the {}.", pts, name));
    }
    if !atari_after.is_empty() {
        hints.push(format!("After this move these groups are in atari: {}.", atari_after.join(", ")));
    }
    if let (Some(p), Some(k)) = (r.policy_prob, r.policy_rank) {
        hints.push(format!("KataGo's network gave the played move {} (its #{} choice before any search).", if p < 0.001 { "under 0.1%".to_string() } else { format!("{:.1}%", p * 100.0) }, k));
    }
    if let (Some(p), Some(k)) = (r.human_prob, r.human_rank) {
        let top = match (&r.human_top, r.human_top_prob) {
            (Some(t), Some(tp)) => format!("; the most common move at that level is {} ({:.0}%)", t, tp * 100.0),
            _ => String::new(),
        };
        hints.push(format!("A player at the chosen human profile plays this move {} of the time (#{}){}.", if p < 0.001 { "under 0.1%".to_string() } else { format!("{:.1}%", p * 100.0) }, k, top));
    }

    TeachingCandidate {
        number: r.number,
        color: r.color,
        mv: r.mv.clone(),
        best,
        point_loss: r.point_loss,
        category: r.category,
        winrate_before: r.winrate_before,
        decided_before,
        opponent_last,
        distance_to_best,
        best_answers_last,
        played_answers_last,
        captured_at,
        loss_region: region.as_ref().map(|(n, _)| n.clone()),
        loss_region_points: region.map(|(_, p)| p).unwrap_or(0.0),
        atari_after,
        policy_prob: r.policy_prob,
        policy_rank: r.policy_rank,
        human_prob: r.human_prob,
        human_rank: r.human_rank,
        human_top: r.human_top.clone(),
        human_top_prob: r.human_top_prob,
        black_stones: stones(&boards[i], Color::Black, sy),
        white_stones: stones(&boards[i], Color::White, sy),
        hints,
    }
}

/// The student's most instructive mistakes: biggest losses first, skipping moves that belong to
/// the same local fight as one already chosen, at most `max` entries.
pub fn select(game: &GameRecord, turns: &[TurnEval], reviews: &[MoveReview], student: Color, max: usize) -> Vec<TeachingCandidate> {
    let boards = boards(game);
    let mut idx: Vec<usize> = reviews
        .iter()
        .enumerate()
        .filter(|(_, r)| r.color == student && r.point_loss >= 1.5 && r.mv != "pass")
        .map(|(i, _)| i)
        .collect();
    idx.sort_by(|a, b| reviews[*b].point_loss.partial_cmp(&reviews[*a].point_loss).unwrap());
    let mut chosen: Vec<usize> = Vec::new();
    for i in idx {
        if chosen.len() >= max {
            break;
        }
        let same_fight = chosen.iter().any(|&j| {
            let close_in_time = reviews[i].number.abs_diff(reviews[j].number) <= 4;
            let close_on_board = match (gtp(&reviews[i].mv, game.size_y), gtp(&reviews[j].mv, game.size_y)) {
                (Some(a), Some(b)) => chebyshev(a, b) <= 2,
                _ => false,
            };
            close_in_time && close_on_board
        });
        if !same_fight {
            chosen.push(i);
        }
    }
    chosen.sort();
    chosen.into_iter().map(|i| build(i, game, turns, reviews, &boards)).collect()
}

/// Moves where the student found KataGo's only good move in a live game.
pub fn praise(reviews: &[MoveReview], student: Color, max: usize) -> Vec<PraiseCandidate> {
    let mut out: Vec<PraiseCandidate> = reviews
        .iter()
        .filter(|r| r.color == student && r.rank == Some(0) && r.mv != "pass" && (0.05..=0.95).contains(&r.winrate_before))
        .filter_map(|r| {
            let first = r.alternatives.first()?;
            let second = r.alternatives.get(1)?;
            if second.visits < 5 {
                return None;
            }
            let sign = if r.color == Color::Black { 1.0 } else { -1.0 };
            let gap = sign * (first.score_lead - second.score_lead);
            if gap >= 2.0 {
                Some(PraiseCandidate {
                    number: r.number,
                    color: r.color,
                    mv: r.mv.clone(),
                    second: second.mv.clone(),
                    gap,
                    winrate_before: r.winrate_before,
                })
            } else {
                None
            }
        })
        .collect();
    out.sort_by(|a, b| b.gap.partial_cmp(&a.gap).unwrap());
    out.truncate(max);
    out.sort_by_key(|p| p.number);
    out
}
