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
    /// Mistake theme assigned by rules (the skill may override with a reason).
    #[serde(default)]
    pub theme: Theme,
    /// The opponent's best punishment after the played move (opponent moves first), and the
    /// score it leads to. Comes from the analysis of the position after the move.
    #[serde(default)]
    pub refutation: Vec<String>,
    #[serde(default)]
    pub refutation_score: f64,
    /// KataGo's line after the better move (mover first).
    #[serde(default)]
    pub better_line: Vec<String>,
    /// Moves in the same area shortly before and after this one, with their point losses:
    /// the cause-and-effect chain the mistake sits in.
    #[serde(default)]
    pub chain_before: Vec<ChainMove>,
    #[serde(default)]
    pub chain_after: Vec<ChainMove>,
    /// Life-and-death changes caused by this move (from the ownership maps).
    #[serde(default)]
    pub status_changes: Vec<StatusChange>,
    pub phase: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Theme {
    Reading,
    LifeAndDeath,
    Tenuki,
    OverDefence,
    Shape,
    Endgame,
    Direction,
    #[default]
    Unknown,
}

impl Theme {
    pub fn label(self) -> &'static str {
        match self {
            Theme::Reading => "reading / tactics",
            Theme::LifeAndDeath => "life and death",
            Theme::Tenuki => "tenuki while threatened",
            Theme::OverDefence => "priority (played locally, bigger move elsewhere)",
            Theme::Shape => "shape / connection",
            Theme::Endgame => "endgame counting",
            Theme::Direction => "direction of play",
            Theme::Unknown => "unclassified",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainMove {
    pub number: usize,
    pub color: Color,
    pub mv: String,
    pub point_loss: f64,
    /// Chebyshev distance from the candidate move.
    pub distance: usize,
    /// "captured at N", "left X in atari", "" ...
    pub note: String,
}

/// A group whose predicted owner changed between the position before and after a move.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusChange {
    pub number: usize,
    pub mover: Color,
    pub group_color: Color,
    pub anchor: String,
    pub stones: usize,
    pub from: String,
    pub to: String,
    pub ownership_before: f32,
    pub ownership_after: f32,
}

/// Facts about one phase of the game, for the arc narrative.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhaseFacts {
    pub name: String,
    pub first_move: usize,
    pub last_move: usize,
    pub winrate_start: f64,
    pub winrate_end: f64,
    pub score_start: f64,
    pub score_end: f64,
    pub student_mean_loss: f64,
    pub opponent_mean_loss: f64,
    pub student_worst: Option<(usize, String, f64)>,
    pub opponent_worst: Option<(usize, String, f64)>,
    /// Regions where ownership changed most over the phase, with points.
    pub hot_regions: Vec<(String, f64)>,
    pub status_changes: Vec<StatusChange>,
    pub student_top1_rate: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PraiseKind {
    /// KataGo's first choice, and every other searched move was clearly worse.
    OnlyMove,
    /// Captured stones or killed a group without losing points.
    Capture,
    /// Turned the student's own group from dead or unsettled to alive.
    Save,
    /// Best move that players at the student's level rarely find.
    NonObvious,
    /// Best move in a position with real alternatives.
    Steady,
}

impl PraiseKind {
    pub fn label(self) -> &'static str {
        match self {
            PraiseKind::OnlyMove => "only good move",
            PraiseKind::Capture => "capture",
            PraiseKind::Save => "saved a group",
            PraiseKind::NonObvious => "non-obvious best move",
            PraiseKind::Steady => "best move",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PraiseCandidate {
    pub number: usize,
    pub color: Color,
    pub mv: String,
    /// The best other searched move.
    pub second: String,
    /// How many points worse the best other searched move was (mover's view).
    pub gap: f64,
    pub winrate_before: f64,
    #[serde(default = "default_kind")]
    pub kind: PraiseKind,
    #[serde(default)]
    pub stones_captured: usize,
    /// Human-policy probability of the move at the student's profile, when known.
    #[serde(default)]
    pub human_prob: Option<f32>,
    /// "a move most 8k players find" etc., filled from the human-profile ladder; None if unknown.
    #[serde(default)]
    pub rating: Option<String>,
    #[serde(default)]
    pub rating_source: Option<String>,
    /// One-line factual reason, safe to quote.
    #[serde(default)]
    pub note: String,
}

fn default_kind() -> PraiseKind {
    PraiseKind::OnlyMove
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

pub fn phase_of(number: usize, total: usize) -> &'static str {
    let (o, m) = if total >= 120 { (50, 150) } else { (total / 4, total * 3 / 4) };
    if number <= o {
        "opening"
    } else if number <= m {
        "middlegame"
    } else {
        "endgame"
    }
}

fn status_of(color: Color, own: f32) -> &'static str {
    let mine = if color == Color::Black { own } else { -own };
    if mine > 0.4 {
        "alive"
    } else if mine < -0.4 {
        "dead"
    } else {
        "unsettled"
    }
}

fn group_mean(own: &[f32], stones: &[Coord], sx: usize) -> f32 {
    if stones.is_empty() {
        return 0.0;
    }
    stones.iter().map(|c| own[c.y * sx + c.x]).sum::<f32>() / stones.len() as f32
}

/// Groups whose life-and-death status changed with each move, from the ownership maps.
pub fn status_changes(game: &GameRecord, turns: &[TurnEval]) -> Vec<StatusChange> {
    let boards = boards(game);
    let (sx, sy) = (game.size_x, game.size_y);
    let mut out = Vec::new();
    for (i, m) in game.moves.iter().enumerate() {
        let (Some(before), Some(after)) = (turns[i].ownership.as_ref(), turns[i + 1].ownership.as_ref()) else { continue };
        if before.len() != sx * sy || after.len() != sx * sy {
            continue;
        }
        for (color, mut stones, _libs) in boards[i + 1].groups() {
            // Only stones that already existed before the move can "change" status.
            stones.retain(|c| boards[i].get(*c) == Some(color));
            if stones.is_empty() {
                continue;
            }
            let ob = group_mean(before, &stones, sx);
            let oa = group_mean(after, &stones, sx);
            let (fs, ts) = (status_of(color, ob), status_of(color, oa));
            let swing = if color == Color::Black { oa - ob } else { ob - oa };
            // Single stones flip constantly in beginner fights; report groups of two or more,
            // and only real changes (a clear status at both ends, or into/out of "dead").
            let meaningful = (fs == "alive" && ts == "dead") || (fs == "dead" && ts == "alive")
                || (stones.len() >= 3 && fs != ts && (fs == "dead" || ts == "dead" || fs == "alive"));
            if stones.len() >= 2 && meaningful && swing.abs() >= 0.5 {
                stones.sort_by_key(|c| (c.y, c.x));
                out.push(StatusChange {
                    number: i + 1,
                    mover: m.color,
                    group_color: color,
                    anchor: stones[0].to_gtp(sy),
                    stones: stones.len(),
                    from: fs.to_string(),
                    to: ts.to_string(),
                    ownership_before: ob,
                    ownership_after: oa,
                });
            }
        }
        // Stones captured by this move. Groups the engine already counted as dead are still
        // recorded (as "captured", from "dead") so the ledger of captures is complete.
        for (color, stones, _l) in boards[i].groups() {
            if stones.iter().all(|c| boards[i + 1].get(*c).is_none()) && color != m.color {
                let ob = group_mean(before, &stones, sx);
                if stones.len() >= 2 || status_of(color, ob) == "alive" {
                    let mut st = stones.clone();
                    st.sort_by_key(|c| (c.y, c.x));
                    out.push(StatusChange {
                        number: i + 1,
                        mover: m.color,
                        group_color: color,
                        anchor: st[0].to_gtp(sy),
                        stones: st.len(),
                        from: status_of(color, ob).to_string(),
                        to: "captured".to_string(),
                        ownership_before: ob,
                        ownership_after: if color == Color::Black { -1.0 } else { 1.0 },
                    });
                }
            }
        }
    }
    out
}

fn ownership_swing_by_region(before: &[f32], after: &[f32], sx: usize, sy: usize) -> Vec<(String, f64)> {
    let mut sums: std::collections::HashMap<&str, f64> = std::collections::HashMap::new();
    for y in 0..sy {
        for x in 0..sx {
            let i = y * sx + x;
            *sums.entry(region_name(Coord { x, y }, sx, sy)).or_insert(0.0) += (after[i] as f64 - before[i] as f64).abs();
        }
    }
    let mut v: Vec<(String, f64)> = sums.into_iter().map(|(k, v)| (k.to_string(), v)).collect();
    v.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
    v
}

/// Per-phase facts for the arc narrative.
pub fn phase_facts(game: &GameRecord, turns: &[TurnEval], reviews: &[MoveReview], changes: &[StatusChange], student: Color) -> Vec<PhaseFacts> {
    let n = game.moves.len();
    let (sx, sy) = (game.size_x, game.size_y);
    let mut out = Vec::new();
    for name in ["opening", "middlegame", "endgame"] {
        let rs: Vec<&MoveReview> = reviews.iter().filter(|r| phase_of(r.number, n) == name).collect();
        let (Some(first), Some(last)) = (rs.first(), rs.last()) else { continue };
        let mean = |c: Color| {
            let v: Vec<f64> = rs.iter().filter(|r| r.color == c).map(|r| r.point_loss.max(0.0)).collect();
            if v.is_empty() { 0.0 } else { v.iter().sum::<f64>() / v.len() as f64 }
        };
        let worst = |c: Color| {
            rs.iter()
                .filter(|r| r.color == c)
                .max_by(|a, b| a.point_loss.partial_cmp(&b.point_loss).unwrap())
                .filter(|r| r.point_loss >= 1.0)
                .map(|r| (r.number, r.mv.clone(), r.point_loss))
        };
        let top1 = {
            let mine: Vec<&&MoveReview> = rs.iter().filter(|r| r.color == student).collect();
            if mine.is_empty() { 0.0 } else { mine.iter().filter(|r| r.rank == Some(0)).count() as f64 / mine.len() as f64 }
        };
        let hot = match (turns[first.number - 1].ownership.as_ref(), turns[last.number].ownership.as_ref()) {
            (Some(b), Some(a)) if b.len() == sx * sy && a.len() == sx * sy => {
                ownership_swing_by_region(b, a, sx, sy).into_iter().filter(|(_, v)| *v >= 2.0).take(3).collect()
            }
            _ => Vec::new(),
        };
        out.push(PhaseFacts {
            name: name.to_string(),
            first_move: first.number,
            last_move: last.number,
            winrate_start: turns[first.number - 1].winrate,
            winrate_end: turns[last.number].winrate,
            score_start: turns[first.number - 1].score_lead,
            score_end: turns[last.number].score_lead,
            student_mean_loss: mean(student),
            opponent_mean_loss: mean(student.opponent()),
            student_worst: worst(student),
            opponent_worst: worst(student.opponent()),
            hot_regions: hot,
            status_changes: changes.iter().filter(|c| c.number >= first.number && c.number <= last.number).cloned().collect(),
            student_top1_rate: top1,
        });
    }
    out
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

fn build(i: usize, game: &GameRecord, turns: &[TurnEval], reviews: &[MoveReview], boards: &[Board], all_changes: &[StatusChange]) -> TeachingCandidate {
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

    // The opponent's best punishment is the top candidate of the position after the move.
    let (refutation, refutation_score) = match turns[i + 1].candidates.first() {
        Some(c) => {
            let mut line = vec![c.mv.clone()];
            line.extend(c.pv.iter().skip(1).take(7).cloned());
            (line, c.score_lead)
        }
        None => (Vec::new(), turns[i + 1].score_lead),
    };
    let better_line: Vec<String> = r.alternatives.first().map(|c| c.pv.iter().take(8).cloned().collect()).unwrap_or_default();

    // Cause-and-effect chain: moves in the same area shortly before and after.
    let chain = |range: std::ops::Range<usize>| -> Vec<ChainMove> {
        range
            .filter(|&j| j != i && j < reviews.len())
            .filter_map(|j| {
                let rj = &reviews[j];
                let pj = gtp(&rj.mv, sy)?;
                let d = chebyshev(pj, played?);
                if d > 3 {
                    return None;
                }
                let mut notes = Vec::new();
                if let Some(k) = (j + 2..=(j + 10).min(game.moves.len())).find(|&k| boards[k].get(pj) != Some(rj.color)) {
                    notes.push(format!("captured at move {}", k));
                }
                for c in all_changes.iter().filter(|c| c.number == rj.number) {
                    notes.push(format!("{} {} ({} stone{}) {} → {}", c.group_color.name(), c.anchor, c.stones, if c.stones == 1 { "" } else { "s" }, c.from, c.to));
                }
                Some(ChainMove { number: rj.number, color: rj.color, mv: rj.mv.clone(), point_loss: rj.point_loss, distance: d, note: notes.join("; ") })
            })
            .collect()
    };
    let mut chain_before = chain(i.saturating_sub(8)..i);
    let mut chain_after = chain(i + 1..(i + 13).min(reviews.len()));
    // Keep the chain readable: the nearest six moves on each side.
    if chain_before.len() > 6 {
        chain_before.drain(..chain_before.len() - 6);
    }
    chain_after.truncate(6);
    let my_changes: Vec<StatusChange> = all_changes.iter().filter(|c| c.number == r.number).cloned().collect();
    let own_group_hurt = my_changes.iter().any(|c| c.group_color == r.color && (c.to == "dead" || c.to == "captured" || c.to == "unsettled"));
    let phase = phase_of(r.number, game.moves.len()).to_string();
    let theme = if own_group_hurt {
        Theme::LifeAndDeath
    } else if captured_at.is_some() || atari_after.iter().any(|a| a.starts_with(r.color.name())) {
        Theme::Reading
    } else if r.stones_captured > 0 || atari_after.iter().any(|a| a.starts_with(r.color.opponent().name())) {
        // Attacked or captured while a bigger move was waiting: a priority problem, not defence.
        Theme::OverDefence
    } else if best_answers_last && !played_answers_last {
        Theme::Tenuki
    } else if played_answers_last && !best_answers_last {
        Theme::OverDefence
    } else if matches!(distance_to_best, Some(d) if d <= 2) {
        Theme::Shape
    } else if phase == "endgame" && r.point_loss < 4.0 {
        Theme::Endgame
    } else if matches!(distance_to_best, Some(d) if d >= 4) {
        Theme::Direction
    } else {
        Theme::Unknown
    };

    let mut hints = Vec::new();
    if decided_before {
        hints.push(format!(
            "The engine strongly favoured one side before this move (Black winrate {:.0}%); use point loss because a saturated winrate can hide large mistakes.",
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
    for c in &my_changes {
        hints.push(format!(
            "This move changed the status of the {} group at {} ({} stone{}): {} → {}.",
            c.group_color.name(),
            c.anchor,
            c.stones,
            if c.stones == 1 { "" } else { "s" },
            c.from,
            c.to
        ));
    }
    if !refutation.is_empty() {
        hints.push(format!(
            "What the move allows: {}'s strongest reply is {} (leading to {}).",
            r.color.opponent().name(),
            refutation.join(" "),
            if refutation_score >= 0.0 { format!("B+{:.1}", refutation_score) } else { format!("W+{:.1}", -refutation_score) }
        ));
    }
    if let (Some(p), Some(k)) = (r.policy_prob, r.policy_rank) {
        hints.push(format!("KataGo's network gave the played move {} (its #{} choice before any search).", if p < 0.001 { "under 0.1%".to_string() } else { format!("{:.1}%", p * 100.0) }, k));
    }
    if let Some(secs) = r.time_spent {
        let fast: Vec<f64> = reviews.iter().filter_map(|x| x.time_spent).collect();
        if fast.len() >= 8 {
            let mut sorted = fast.clone();
            sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let q1 = sorted[sorted.len() / 4];
            if secs <= q1 {
                hints.push(format!("Played in {:.0} s, among the fastest quarter of the game's moves: likely not read out.", secs));
            } else if secs >= sorted[sorted.len() * 3 / 4] {
                hints.push(format!("Played after {:.0} s of thought, among the slowest quarter: a considered decision that still went wrong.", secs));
            }
        }
    }
    if let (Some(p), Some(k)) = (r.target_prob, r.target_rank) {
        let top = match (&r.target_top, r.target_top_prob) {
            (Some(t), Some(tp)) => format!("; their most common move here is {} ({:.0}%)", t, tp * 100.0),
            _ => String::new(),
        };
        hints.push(format!("Players at the stronger target profile play this move {} of the time (#{}){}.", if p < 0.001 { "under 0.1%".to_string() } else { format!("{:.1}%", p * 100.0) }, k, top));
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
        theme,
        refutation,
        refutation_score,
        better_line,
        chain_before,
        chain_after,
        status_changes: my_changes,
        phase,
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
    let changes = status_changes(game, turns);
    chosen.into_iter().map(|i| build(i, game, turns, reviews, &boards, &changes)).collect()
}

/// Moves worth praising. No winrate window: a decided game still has good moves. The gap is
/// measured against the best *other* searched move by score, whatever its visit count (a move
/// KataGo abandoned after one visit was judged worse, not left unjudged). Kinds, in priority
/// order: capture / save, only good move, non-obvious best move, steady best move.
pub fn praise(reviews: &[MoveReview], changes: &[StatusChange], student: Color, max: usize) -> Vec<PraiseCandidate> {
    fn build(r: &MoveReview, changes: &[StatusChange], relaxed: bool) -> Option<PraiseCandidate> {
        if r.color != student_of(r) || r.mv == "pass" {
            return None;
        }
        let sign = if r.color == Color::Black { 1.0 } else { -1.0 };
        let played_score = r.played_candidate.as_ref().map(|c| c.score_lead).unwrap_or(r.score_after);
        let others: Vec<&crate::analysis::Candidate> = r.alternatives.iter().filter(|c| c.mv != r.mv).collect();
        let best_other = others
            .iter()
            .max_by(|a, b| (sign * a.score_lead).partial_cmp(&(sign * b.score_lead)).unwrap())
            .copied();
        let gap = best_other.map(|c| sign * (played_score - c.score_lead)).unwrap_or(0.0);
        let second = best_other.map(|c| c.mv.clone()).unwrap_or_default();
        let is_best = r.rank == Some(0) || r.point_loss <= 0.5;
        let killed = changes.iter().any(|c| c.number == r.number && c.group_color != r.color && (c.to == "dead" || c.to == "captured") && c.from != "dead");
        let saved = changes.iter().any(|c| c.number == r.number && c.group_color == r.color && c.to == "alive" && c.from != "alive");
        let human_low = r.human_prob.map_or(false, |p| p < 0.25);
        // A big capture tolerates a little imprecision: up to 1 point plus a quarter point per stone (cap 3).
        let capture_tolerance = (1.0 + 0.25 * r.stones_captured as f64).min(3.0);
        let (kind, note) = if (r.stones_captured >= 3 || killed) && r.point_loss <= capture_tolerance {
            (PraiseKind::Capture, if r.stones_captured > 0 { format!("captured {} stone{} without losing points", r.stones_captured, if r.stones_captured == 1 { "" } else { "s" }) } else { "killed a group without losing points".to_string() })
        } else if saved && r.point_loss <= 1.0 {
            (PraiseKind::Save, "brought the student's own group back to life".to_string())
        } else if is_best && best_other.is_some() && gap >= if relaxed { 1.5 } else { 2.0 } {
            (PraiseKind::OnlyMove, format!("KataGo's first choice; the best other searched move ({}) was {:.1} points worse", second, gap))
        } else if is_best && human_low && (gap >= 1.0 || r.stones_captured > 0) {
            (PraiseKind::NonObvious, format!("KataGo's first choice, yet players at the student's level choose it only {:.0}% of the time", r.human_prob.unwrap_or(0.0) * 100.0))
        } else if r.rank == Some(0) && r.point_loss <= 0.3 && gap >= if relaxed { 0.5 } else { 1.0 } {
            (PraiseKind::Steady, format!("KataGo's first choice, {:.1} points ahead of the next searched move", gap))
        } else {
            return None;
        };
        Some(PraiseCandidate {
            number: r.number,
            color: r.color,
            mv: r.mv.clone(),
            second,
            gap,
            winrate_before: r.winrate_before,
            kind,
            stones_captured: r.stones_captured,
            human_prob: r.human_prob,
            rating: None,
            rating_source: None,
            note,
        })
    }
    fn student_of(r: &MoveReview) -> Color {
        r.color
    }
    let score = |p: &PraiseCandidate| match p.kind {
        PraiseKind::Capture => 100.0 + p.stones_captured as f64,
        PraiseKind::Save => 80.0,
        PraiseKind::OnlyMove => 50.0 + p.gap.min(30.0),
        PraiseKind::NonObvious => 40.0 + (1.0 - p.human_prob.unwrap_or(0.5) as f64) * 20.0,
        PraiseKind::Steady => p.gap.min(10.0),
    };
    let mine: Vec<&MoveReview> = reviews.iter().filter(|r| r.color == student).collect();
    let mut out: Vec<PraiseCandidate> = mine.iter().filter_map(|r| build(r, changes, false)).collect();
    if out.len() < 3 {
        // Relax the gap thresholds so the student always gets a few things done well, when any exist.
        for r in &mine {
            if out.iter().all(|p| p.number != r.number) {
                if let Some(p) = build(r, changes, true) {
                    out.push(p);
                }
            }
        }
    }
    out.sort_by(|a, b| score(b).partial_cmp(&score(a)).unwrap());
    out.truncate(max);
    out.sort_by_key(|p| p.number);
    out
}
