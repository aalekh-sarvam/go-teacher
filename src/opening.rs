//! Opening patterns per corner, described from the moves themselves and judged by KataGo:
//! which corner point was taken, how the opponent approached or invaded, and the first move in
//! the corner sequence that KataGo disliked. No hand-written joseki library is involved.

use crate::analysis::MoveReview;
use crate::sgf::{Color, Coord, GameRecord};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CornerMove {
    pub number: usize,
    pub color: Color,
    pub mv: String,
    /// "4-4 point", "small knight's approach", "3-3 invasion", ...
    pub description: String,
    pub point_loss: f64,
    pub rank: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CornerPattern {
    pub corner: String,
    pub moves: Vec<CornerMove>,
    /// First move in this corner that lost 1.5+ points against KataGo's choice.
    pub first_deviation: Option<(usize, String, String, f64)>,
    /// A searchable name for the pattern, e.g. "4-4 point, small knight's approach, 3-3 invasion".
    pub summary: String,
}

fn corner_of(c: Coord, sx: usize, sy: usize) -> Option<(&'static str, usize, usize)> {
    // Distances from the nearest edges, 1-based; only stones within 6 lines of both edges count.
    let dx = (c.x + 1).min(sx - c.x);
    let dy = (c.y + 1).min(sy - c.y);
    if dx > 6 || dy > 6 {
        return None;
    }
    let left = c.x + 1 <= sx - c.x;
    let top = c.y + 1 <= sy - c.y;
    let name = match (left, top) {
        (true, true) => "upper left",
        (false, true) => "upper right",
        (true, false) => "lower left",
        (false, false) => "lower right",
    };
    Some((name, dx, dy))
}

fn point_name(dx: usize, dy: usize) -> String {
    let (a, b) = (dx.min(dy), dx.max(dy));
    format!("{}-{} point", a, b)
}

fn relation(from: (usize, usize), to: (usize, usize)) -> &'static str {
    let (ax, ay) = ((from.0 as i32 - to.0 as i32).abs(), (from.1 as i32 - to.1 as i32).abs());
    match (ax.min(ay), ax.max(ay)) {
        (0, 1) => "attachment",
        (1, 1) => "diagonal attachment",
        (0, 2) => "one-space jump / high approach",
        (1, 2) => "small knight's approach",
        (0, 3) => "two-space jump",
        (1, 3) => "large knight's approach",
        (2, 2) => "diagonal jump",
        (2, 3) => "large knight's move",
        _ => "extension in the corner",
    }
}

/// Describe up to eight moves in each corner of the opening (first 50 moves, boards of 13+ lines).
pub fn corner_patterns(game: &GameRecord, reviews: &[MoveReview]) -> Vec<CornerPattern> {
    if game.size_x < 13 || game.size_y < 13 {
        return Vec::new();
    }
    let mut corners: std::collections::BTreeMap<&'static str, Vec<(usize, Color, Coord, usize, usize)>> = Default::default();
    for (i, m) in game.moves.iter().enumerate().take(50) {
        let Some(p) = m.point else { continue };
        if let Some((name, dx, dy)) = corner_of(p, game.size_x, game.size_y) {
            let v = corners.entry(name).or_default();
            if v.len() < 8 {
                v.push((i, m.color, p, dx, dy));
            }
        }
    }
    let mut out = Vec::new();
    for (corner, list) in corners {
        if list.is_empty() {
            continue;
        }
        let first_color = list[0].1;
        let first_pt = (list[0].3, list[0].4);
        let mut moves = Vec::new();
        let mut names: Vec<String> = Vec::new();
        let mut first_deviation = None;
        for (k, (i, color, p, dx, dy)) in list.iter().enumerate() {
            let r = &reviews[*i];
            let description = if k == 0 {
                point_name(*dx, *dy)
            } else if *color != first_color && (*dx, *dy) == (3, 3) && first_pt != (3, 3) {
                "3-3 invasion".to_string()
            } else if *color != first_color && k == list.iter().position(|x| x.1 != first_color).unwrap_or(usize::MAX) {
                format!("{} ({})", relation(first_pt, (*dx, *dy)), point_name(*dx, *dy))
            } else {
                // Relate to the previous stone in this corner.
                let prev = &list[k - 1];
                format!("{} to the previous stone ({})", relation((prev.3, prev.4), (*dx, *dy)), point_name(*dx, *dy))
            };
            if k < 3 || description.contains("invasion") {
                names.push(description.clone());
            }
            if first_deviation.is_none() && r.point_loss >= 1.5 {
                let best = r.alternatives.first().map(|c| c.mv.clone()).unwrap_or_default();
                first_deviation = Some((r.number, r.mv.clone(), best, r.point_loss));
            }
            moves.push(CornerMove {
                number: r.number,
                color: *color,
                mv: p.to_gtp(game.size_y),
                description,
                point_loss: r.point_loss,
                rank: r.rank,
            });
        }
        out.push(CornerPattern { corner: corner.to_string(), moves, first_deviation, summary: names.join(", ") });
    }
    out
}
