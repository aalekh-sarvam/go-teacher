//! Renders a [`GameAnalysis`] into a long Markdown document meant to be read by an AI teacher.

use crate::analysis::{Candidate, Category, GameAnalysis, MoveReview};
use crate::sgf::Color as C;
use std::collections::HashSet;
use crate::board::Board;
use crate::sgf::{Color, Coord};
use std::collections::HashMap;
use std::fmt::Write;

/// Bump when the structure of the report changes; the lesson skill's parser checks it.
pub const REPORT_FORMAT: u32 = 5;

fn pct(x: f64) -> String {
    format!("{:.1}%", x * 100.0)
}

fn lead(x: f64) -> String {
    if x >= 0.0 {
        format!("B+{:.1}", x)
    } else {
        format!("W+{:.1}", -x)
    }
}

fn signed(x: f64) -> String {
    let x = if x.abs() < 0.05 { 0.0 } else { x };
    format!("{:+.1}", x)
}

fn one_dec(x: f64) -> String {
    let x = if x.abs() < 0.05 { 0.0 } else { x };
    format!("{:.1}", x)
}

fn opt(s: &Option<String>) -> &str {
    s.as_deref().unwrap_or("—")
}

fn player_label(a: &GameAnalysis, c: Color) -> String {
    let (name, rank) = match c {
        Color::Black => (&a.game.player_black, &a.game.rank_black),
        Color::White => (&a.game.player_white, &a.game.rank_white),
    };
    match (name, rank) {
        (Some(n), Some(r)) => format!("{} ({}, {})", c.name(), n, r),
        (Some(n), None) => format!("{} ({})", c.name(), n),
        (None, Some(r)) => format!("{} ({})", c.name(), r),
        (None, None) => c.name().to_string(),
    }
}

struct PlayerStats {
    moves: usize,
    total_loss: f64,
    mean_loss: f64,
    median_loss: f64,
    top1: usize,
    top3: usize,
    counts: HashMap<Category, usize>,
}

fn stats_for<'a>(reviews: impl Iterator<Item = &'a MoveReview>) -> PlayerStats {
    let mut losses: Vec<f64> = Vec::new();
    let mut s = PlayerStats {
        moves: 0,
        total_loss: 0.0,
        mean_loss: 0.0,
        median_loss: 0.0,
        top1: 0,
        top3: 0,
        counts: HashMap::new(),
    };
    for r in reviews {
        s.moves += 1;
        let l = r.point_loss.max(0.0);
        losses.push(l);
        s.total_loss += l;
        if r.rank == Some(0) {
            s.top1 += 1;
        }
        if matches!(r.rank, Some(k) if k < 3) {
            s.top3 += 1;
        }
        *s.counts.entry(r.category).or_insert(0) += 1;
    }
    if s.moves > 0 {
        s.mean_loss = s.total_loss / s.moves as f64;
        losses.sort_by(|a, b| a.partial_cmp(b).unwrap());
        s.median_loss = losses[losses.len() / 2];
    }
    s
}

fn phase_of(number: usize, total: usize) -> &'static str {
    // Fixed cut-offs for a normal 19x19 game, compressed for very short games.
    let (o, m) = if total >= 120 { (50, 150) } else { (total / 4, total * 3 / 4) };
    if number <= o {
        "opening"
    } else if number <= m {
        "middlegame"
    } else {
        "endgame"
    }
}

fn bar(winrate: f64) -> String {
    let filled = (winrate * 20.0).round().clamp(0.0, 20.0) as usize;
    format!("{}{}", "█".repeat(filled), "░".repeat(20 - filled))
}

const CAND_HEADER: &str = "| # | Move | Black winrate | Score | Loss vs best | Visits | Policy | Expected continuation |\n|---|---|---|---|---|---|---|---|";

fn candidate_row(c: &Candidate, played: &str, best_score: f64, mover: C) -> String {
    let mark = if c.mv == played { " ← played" } else { "" };
    let sign = if mover == C::Black { 1.0 } else { -1.0 };
    let loss = sign * (best_score - c.score_lead);
    let pv: Vec<&str> = c.pv.iter().take(6).map(|s| s.as_str()).collect();
    format!(
        "| {} | {}{} | {} | {} | {} | {} | {:.1}% | {} |",
        c.order + 1,
        c.mv,
        mark,
        pct(c.winrate),
        lead(c.score_lead),
        if loss.abs() < 0.05 { "best".to_string() } else { format!("{:.1}", loss) },
        c.visits,
        c.prior * 100.0,
        if pv.is_empty() { "—".to_string() } else { pv.join(" ") }
    )
}

fn prob(p: f32) -> String {
    if p < 0.001 { "<0.1%".to_string() } else { format!("{:.1}%", p * 100.0) }
}

fn is_auto_comment(c: &str) -> bool {
    // KaTrain writes "Move 13: B G3\nScore: ..." on every move; it duplicates the analysis.
    c.starts_with("Move ") && c[5..].chars().take(4).any(|ch| ch == ':')
}

fn diagram(board: &Board, review: &MoveReview, size_y: usize) -> String {
    let mut marks: HashMap<Coord, char> = HashMap::new();
    let mut legend: Vec<String> = Vec::new();
    if let Some(c) = Coord::from_gtp(&review.mv, size_y) {
        let ch = match review.color {
            Color::Black => '#',
            Color::White => '@',
        };
        marks.insert(c, ch);
        legend.push(format!("`{}` = {} {} (the move just played)", ch, review.color.name(), review.mv));
    }
    let mut n = 0;
    for alt in review.alternatives.iter() {
        if alt.mv == review.mv || alt.mv == "pass" {
            continue;
        }
        if n >= 3 {
            break;
        }
        if let Some(c) = Coord::from_gtp(&alt.mv, size_y) {
            if board.get(c).is_none() && !marks.contains_key(&c) {
                let ch = char::from(b'1' + n as u8);
                marks.insert(c, ch);
                legend.push(format!("`{}` = KataGo alternative {} ({}, {})", ch, alt.mv, pct(alt.winrate), lead(alt.score_lead)));
                n += 1;
            }
        }
    }
    let mut s = String::new();
    s.push_str("```\n");
    s.push_str(&board.render(&marks));
    s.push_str("```\n");
    s.push_str("`X` = Black, `O` = White, `+` = star point. ");
    s.push_str(&legend.join("; "));
    s.push_str(".\n");
    s
}

pub fn render_detailed_markdown(a: &GameAnalysis) -> String {
    let mut out = String::new();
    let g = &a.game;
    let n = g.moves.len();
    let w = &mut out;

    // ------------------------------------------------------------------ title
    let title = g
        .game_name
        .clone()
        .unwrap_or_else(|| format!("{} vs {}", opt(&g.player_black), opt(&g.player_white)));
    let _ = writeln!(w, "# Go game review: {}\n", title);
    let _ = writeln!(w, "Generated by go_teacher using {} on {}.\n", a.engine_version, a.started_at);
    let _ = writeln!(w, "Report format: {}\n", 4);

    let _ = writeln!(w, "## Game information\n");
    let _ = writeln!(w, "| Field | Value |");
    let _ = writeln!(w, "|---|---|");
    let _ = writeln!(w, "| Black | {} |", player_label(a, Color::Black));
    let _ = writeln!(w, "| White | {} |", player_label(a, Color::White));
    let _ = writeln!(w, "| Result recorded in SGF | {} |", opt(&g.result));
    let _ = writeln!(w, "| Date | {} |", opt(&g.date));
    let _ = writeln!(w, "| Event | {} |", opt(&g.event));
    let _ = writeln!(w, "| Place | {} |", opt(&g.place));
    let _ = writeln!(w, "| Board size | {}x{} |", g.size_x, g.size_y);
    let _ = writeln!(w, "| Handicap | {} |", g.handicap.map(|h| h.to_string()).unwrap_or_else(|| if g.setup_black.is_empty() { "0".into() } else { format!("{} setup stones", g.setup_black.len()) }));
    let _ = writeln!(w, "| Komi used for analysis | {} |", a.komi);
    let _ = writeln!(w, "| Rules used for analysis | {} (SGF says: {}) |", a.rules, opt(&g.rules));
    let _ = writeln!(w, "| Time settings | {} / {} |", opt(&g.time_settings), opt(&g.overtime));
    let _ = writeln!(w, "| Number of moves | {} |", n);
    let _ = writeln!(w, "| Student | {} ({}) |", a.student.map(|c| c.name()).unwrap_or("Black"), a.student_reason);
    let _ = writeln!(w, "| Analysis strength | {} |", a.visits_setting);
    if !a.deepened.is_empty() {
        let _ = writeln!(w, "| Deeply re-analysed positions | {} (marked ◆ in the move-by-move section) |", a.deepened.len());
    }
    let _ = writeln!(w, "| Analysis wall time | {:.0} s |", a.elapsed_seconds);
    let _ = writeln!(w);
    if let Some(gc) = &g.game_comment {
        let _ = writeln!(w, "Game comment from the SGF:\n\n> {}\n", gc.replace('\n', "\n> "));
    }
    if !g.warnings.is_empty() || !a.engine_warnings.is_empty() {
        let _ = writeln!(w, "Warnings while reading or analysing the file:\n");
        for wmsg in g.warnings.iter().chain(a.engine_warnings.iter()) {
            let _ = writeln!(w, "- {}", wmsg);
        }
        let _ = writeln!(w);
    }

    // --------------------------------------------------------- reading guide
    let _ = writeln!(w, "## How to read this document\n");
    let _ = writeln!(w, "This report was produced automatically by the KataGo engine; it contains numbers, not explanations. \
Your job as the teacher is to turn these numbers into concrete, stylistic feedback for the players.\n");
    let _ = writeln!(w, "- **Coordinates** use the GTP convention: columns `A`–`T` from left to right (the letter `I` is skipped), \
rows `1`–`{}` from bottom to top. `D4` is the lower-left 4-4 point, `Q16` the upper-right 4-4 point. `pass` is a pass.", g.size_y);
    let _ = writeln!(w, "- **Winrate** is always Black's probability of winning as estimated by KataGo (0–100%). \
A White winrate is 100% minus this value.");
    let _ = writeln!(w, "- **Score lead** is KataGo's expected final margin, `B+x` if Black leads by x points, `W+x` if White leads.");
    let _ = writeln!(w, "- **Point loss** for a move is how many points the mover gave away compared with KataGo's evaluation before the move \
(before-move lead minus after-move lead, from the mover's perspective). Small negative values are search noise, not brilliance.");
    let _ = writeln!(w, "- **Categories** by point loss: best/excellent < 0.5, good < 1.5, inaccuracy < 3, mistake < 6, big mistake < 12, blunder ≥ 12.");
    let _ = writeln!(w, "- **Rank** is where the played move sits in KataGo's candidate list (#1 = KataGo's top choice). \
“not searched” means KataGo did not consider the move at all, which usually signals an unnatural shape or direction.");
    let _ = writeln!(w, "- **PV** (principal variation) is the continuation KataGo expects after a candidate move, alternating colours starting with the mover.");
    let _ = writeln!(w, "- **Diagrams** show the position *after* the move under discussion, drawn for every mistake of at least 3 points and at regular checkpoints. \
`X` is Black, `O` is White, `#`/`@` mark the Black/White stone just played, digits mark KataGo's preferred alternatives.");
    let _ = writeln!(w, "- **Teaching candidates** (next section) is the shortlist: the student's most instructive mistakes with board facts computed by the program \
(where the points went, captures, atari, whether the better move answers the opponent's last move) and a puzzle-ready stone list. Start there.");
    let _ = writeln!(w, "- **Loss vs best** in a candidate table is how many points worse that move is than KataGo's first choice, from the mover's view; use it to grade alternatives.");
    let _ = writeln!(w, "- **Policy** numbers: the network's move probability before any search. **Human policy** (when a profile was chosen) is how often a player of that rank plays the move; \
a high human policy with a large loss means a typical mistake for the level, a low one means an unusual move.");
    let _ = writeln!(w, "- **Theme** is a rule-based classification of each teaching candidate (reading, life and death, tenuki, over-defending, shape, endgame, direction). \
**What the move allows** is the opponent's strongest punishment after the played move, taken from the analysis of the next position; **Better line** is KataGo's continuation after its preferred move. \
**Chain** lists the moves played in the same area shortly before and after, with their losses, so a lesson can explain how the position arose and what the mistake led to.");
    let _ = writeln!(w, "- **Opening patterns** names how each corner was played (corner point, approach, invasion) with KataGo's verdict per move; \
**Compared with the student's earlier games** appears once the student has two or more earlier analysed games. **Time spent** lines appear when the record has clock data. \
**Target profile** numbers are the human-style policy at a stronger rank chosen at upload.");
    let _ = writeln!(w, "- **Game arc facts** gives per-phase numbers (winrate and score at the phase boundaries, mean loss per side, worst move per side, where the board changed most) \
and every life-and-death change detected from the ownership maps, as material for a phase-by-phase narrative.");
    let _ = writeln!(w, "- Only the key moves (teaching candidates, losses of 2+ points, praised moves, the last move) have full entries with a candidate table and diagram; \
every other move is one line. The appendix lists every move.");
    let _ = writeln!(w, "- When teaching, group mistakes by theme (direction of play, reading, shape, endgame counting, timing of invasions) and by phase, \
and cite move numbers so the student can find them in the game record.\n");

    // --------------------------------------------------------------- summary
    let _ = writeln!(w, "## Summary\n");
    if let (Some(first), Some(last)) = (a.turns.first(), a.turns.last()) {
        let _ = writeln!(
            w,
            "KataGo's estimate at the start of the game: Black winrate {}, score {}. At the end of the record: Black winrate {}, score {}. \
Recorded result: {}.\n",
            pct(first.winrate),
            lead(first.score_lead),
            pct(last.winrate),
            lead(last.score_lead),
            opt(&g.result)
        );
    }

    let sb = stats_for(a.reviews.iter().filter(|r| r.color == Color::Black));
    let sw = stats_for(a.reviews.iter().filter(|r| r.color == Color::White));
    let _ = writeln!(w, "### Accuracy by player\n");
    let _ = writeln!(w, "| Metric | {} | {} |", player_label(a, Color::Black), player_label(a, Color::White));
    let _ = writeln!(w, "|---|---|---|");
    let _ = writeln!(w, "| Moves played | {} | {} |", sb.moves, sw.moves);
    let _ = writeln!(w, "| Mean point loss per move | {:.2} | {:.2} |", sb.mean_loss, sw.mean_loss);
    let _ = writeln!(w, "| Median point loss per move | {:.2} | {:.2} |", sb.median_loss, sw.median_loss);
    let _ = writeln!(w, "| Total points lost | {:.1} | {:.1} |", sb.total_loss, sw.total_loss);
    let rate = |k: usize, m: usize| if m == 0 { "—".to_string() } else { format!("{:.0}%", 100.0 * k as f64 / m as f64) };
    let _ = writeln!(w, "| Matched KataGo's top choice | {} | {} |", rate(sb.top1, sb.moves), rate(sw.top1, sw.moves));
    let _ = writeln!(w, "| Within KataGo's top 3 | {} | {} |", rate(sb.top3, sb.moves), rate(sw.top3, sw.moves));
    for c in Category::all() {
        let _ = writeln!(
            w,
            "| {} | {} | {} |",
            c.label(),
            sb.counts.get(&c).copied().unwrap_or(0),
            sw.counts.get(&c).copied().unwrap_or(0)
        );
    }
    let _ = writeln!(w);

    // phases
    let _ = writeln!(w, "### Point loss by phase\n");
    let _ = writeln!(w, "| Phase | Moves | Black mean loss | Black total | White mean loss | White total |");
    let _ = writeln!(w, "|---|---|---|---|---|---|");
    for phase in ["opening", "middlegame", "endgame"] {
        let in_phase: Vec<&MoveReview> = a.reviews.iter().filter(|r| phase_of(r.number, n) == phase).collect();
        if in_phase.is_empty() {
            continue;
        }
        let range = format!("{}–{}", in_phase.first().unwrap().number, in_phase.last().unwrap().number);
        let pb = stats_for(in_phase.iter().copied().filter(|r| r.color == Color::Black));
        let pw = stats_for(in_phase.iter().copied().filter(|r| r.color == Color::White));
        let _ = writeln!(
            w,
            "| {} | {} | {:.2} | {:.1} | {:.2} | {:.1} |",
            phase, range, pb.mean_loss, pb.total_loss, pw.mean_loss, pw.total_loss
        );
    }
    let _ = writeln!(w);

    // game flow
    let _ = writeln!(w, "### Game flow (Black winrate and score lead after each move)\n");
    let _ = writeln!(w, "| After move | Black winrate | Score lead | |");
    let _ = writeln!(w, "|---|---|---|---|");
    for t in a.turns.iter() {
        if t.turn % 10 == 0 || t.turn == n {
            let _ = writeln!(w, "| {} | {} | {} | `{}` |", t.turn, pct(t.winrate), lead(t.score_lead), bar(t.winrate));
        }
    }
    let _ = writeln!(w);

    // time and loss (when the record has clocks)
    let timed: Vec<&MoveReview> = a.reviews.iter().filter(|r| r.time_spent.is_some()).collect();
    if timed.len() >= 8 {
        let mut secs: Vec<f64> = timed.iter().map(|r| r.time_spent.unwrap()).collect();
        secs.sort_by(|x, y| x.partial_cmp(y).unwrap());
        let median = secs[secs.len() / 2];
        let _ = writeln!(w, "### Time and loss\n");
        let _ = writeln!(w, "The record has clock data. Moves are split at the median thinking time ({:.0} s).\n", median);
        let _ = writeln!(w, "| Player | Fast moves (≤ median) | Mean loss | Slow moves | Mean loss |");
        let _ = writeln!(w, "|---|---|---|---|---|");
        for color in [C::Black, C::White] {
            let fast: Vec<&&MoveReview> = timed.iter().filter(|r| r.color == color && r.time_spent.unwrap() <= median).collect();
            let slow: Vec<&&MoveReview> = timed.iter().filter(|r| r.color == color && r.time_spent.unwrap() > median).collect();
            let mean = |v: &[&&MoveReview]| if v.is_empty() { 0.0 } else { v.iter().map(|r| r.point_loss.max(0.0)).sum::<f64>() / v.len() as f64 };
            let _ = writeln!(w, "| {} | {} | {:.2} | {} | {:.2} |", color.name(), fast.len(), mean(&fast), slow.len(), mean(&slow));
        }
        let _ = writeln!(w);
    }

    // biggest mistakes
    let _ = writeln!(w, "### Biggest mistakes\n");
    for color in [Color::Black, Color::White] {
        let mut worst: Vec<&MoveReview> = a.reviews.iter().filter(|r| r.color == color && r.point_loss >= 1.5).collect();
        worst.sort_by(|x, y| y.point_loss.partial_cmp(&x.point_loss).unwrap());
        let _ = writeln!(w, "**{}**\n", player_label(a, color));
        if worst.is_empty() {
            let _ = writeln!(w, "No move lost more than 1.5 points.\n");
            continue;
        }
        let _ = writeln!(w, "| Move | Played | Loss (pts) | Winrate change | Category | KataGo preferred | Phase |");
        let _ = writeln!(w, "|---|---|---|---|---|---|---|");
        for r in worst.iter().take(12) {
            let best = r.alternatives.first().map(|c| c.mv.clone()).unwrap_or_else(|| "—".into());
            let _ = writeln!(
                w,
                "| {} | {} | {:.1} | {} → {} | {} | {} | {} |",
                r.number,
                r.mv,
                r.point_loss,
                pct(r.winrate_before),
                pct(r.winrate_after),
                r.category.label(),
                best,
                phase_of(r.number, n)
            );
        }
        let _ = writeln!(w);
    }

    // turning points
    let _ = writeln!(w, "### Turning points\n");
    let mut any = false;
    for r in &a.reviews {
        let crossed = (r.winrate_before - 0.5) * (r.winrate_after - 0.5) < 0.0 && (r.winrate_before - r.winrate_after).abs() >= 0.05;
        let swing = (r.winrate_before - r.winrate_after).abs() >= 0.15;
        // A swing measured on the shallow pass is noise unless the move also cost real points.
        // Winrate swings on a near-even board are search noise unless real points moved too.
        let deep_both = a.deepened.contains(&(r.number - 1)) && a.deepened.contains(&r.number);
        let reliable = r.point_loss.abs() >= 3.0 || (deep_both && r.point_loss.abs() >= 1.5);
        if (crossed || swing) && reliable {
            any = true;
            let _ = writeln!(
                w,
                "- Move {} ({} {}): Black winrate {} → {}, score {} → {}{}.",
                r.number,
                r.color.name(),
                r.mv,
                pct(r.winrate_before),
                pct(r.winrate_after),
                lead(r.score_before),
                lead(r.score_after),
                if crossed { " — the lead changed hands" } else { "" }
            );
        }
    }
    if !any {
        let _ = writeln!(w, "No single move swung the winrate by 15% or changed who was ahead.");
    }
    let _ = writeln!(w);

    // ------------------------------------------------------- teaching candidates
    let student = a.student.unwrap_or(Color::Black);
    let _ = writeln!(w, "## Teaching candidates\n");
    let _ = writeln!(
        w,
        "Student: **{}** ({}). Winrates in the table are Black's; the game is *decided* when the winrate before the move was already outside 5–95%, \
in which case the winrate swing is meaningless and only the point loss counts.\n",
        player_label(a, student),
        a.student_reason
    );
    if a.teaching.is_empty() {
        let _ = writeln!(w, "No move by the student lost 1.5 points or more.\n");
    } else {
        let _ = writeln!(w, "| Move | Played | Better | Loss | Theme | Decided? | Where the points went | Better move is | Stone captured? | Net policy | Human policy |");
        let _ = writeln!(w, "|---|---|---|---|---|---|---|---|---|---|---|");
        for t in &a.teaching {
            let where_ = match &t.loss_region {
                Some(r) => format!("{} (~{:.0} pts)", r, t.loss_region_points),
                None => "—".to_string(),
            };
            let better_is = match t.distance_to_best {
                Some(d) if d <= 2 => format!("same area (dist {})", d),
                Some(d) => format!("elsewhere (dist {})", d),
                None => "—".to_string(),
            };
            let captured = t.captured_at.map(|k| format!("yes, move {}", k)).unwrap_or_else(|| "no".into());
            let pol = match (t.policy_prob, t.policy_rank) {
                (Some(p), Some(r)) => format!("{} (#{})", prob(p), r),
                _ => "—".into(),
            };
            let hum = match (t.human_prob, t.human_rank) {
                (Some(p), Some(r)) => format!("{} (#{})", prob(p), r),
                _ => "—".into(),
            };
            let _ = writeln!(
                w,
                "| {} | {} | {} | {:.1} | {} | {} | {} | {} | {} | {} | {} |",
                t.number,
                t.mv,
                t.best.as_deref().unwrap_or("—"),
                t.point_loss,
                t.theme.label(),
                if t.decided_before { format!("yes ({})", pct(t.winrate_before)) } else { "no".into() },
                where_,
                better_is,
                captured,
                pol,
                hum
            );
        }
        let _ = writeln!(w);
        for t in &a.teaching {
            let _ = writeln!(w, "### Candidate: move {} ({} {})\n", t.number, t.color.name(), t.mv);
            for h in &t.hints {
                let _ = writeln!(w, "- {}", h);
            }
            let _ = writeln!(
                w,
                "- Position before the move ({} to play; opponent's last move {}): Black stones {}; White stones {}.",
                t.color.name(),
                t.opponent_last.as_deref().unwrap_or("none"),
                if t.black_stones.is_empty() { "none".to_string() } else { t.black_stones.join(" ") },
                if t.white_stones.is_empty() { "none".to_string() } else { t.white_stones.join(" ") }
            );
            if !t.better_line.is_empty() {
                let _ = writeln!(w, "- Better line ({} first): {}", t.color.name(), t.better_line.join(" "));
            }
            if !t.refutation.is_empty() {
                let _ = writeln!(
                    w,
                    "- What the move allows ({} first): {} → {}",
                    t.color.opponent().name(),
                    t.refutation.join(" "),
                    lead(t.refutation_score)
                );
            }
            if !t.chain_before.is_empty() || !t.chain_after.is_empty() {
                let fmt = |c: &crate::teaching::ChainMove| {
                    format!(
                        "{} {} {} (loss {}{})",
                        c.number,
                        c.color.name(),
                        c.mv,
                        one_dec(c.point_loss),
                        if c.note.is_empty() { String::new() } else { format!("; {}", c.note) }
                    )
                };
                let before: Vec<String> = t.chain_before.iter().map(fmt).collect();
                let after: Vec<String> = t.chain_after.iter().map(fmt).collect();
                let _ = writeln!(
                    w,
                    "- Chain in this area — before: {}; after: {}.",
                    if before.is_empty() { "none".to_string() } else { before.join(", ") },
                    if after.is_empty() { "none".to_string() } else { after.join(", ") }
                );
            }
            let _ = writeln!(w, "- Theme: {}. Phase: {}.", t.theme.label(), t.phase);
            let _ = writeln!(w, "- Full entry: see \"Move {}\" below.\n", t.number);
        }
    }
    let _ = writeln!(w, "### Good moves worth praising\n");
    if a.praise.is_empty() {
        let captures = a.reviews.iter().filter(|r| r.color == student && r.stones_captured > 0).count();
        if captures > 0 {
            let _ = writeln!(w, "No move met the praise thresholds, although the student captured stones on {} move(s) (see the compact move list).\n", captures);
        } else {
            let _ = writeln!(w, "No move by the student met the praise thresholds in this game.\n");
        }
    } else {
        let _ = writeln!(w, "Moves worth praising: captures and saves without losing points, KataGo's first choice when every other searched move was clearly worse, and best moves players at the student's level rarely find.\n");
        let _ = writeln!(w, "| Move | Played | Kind | Why | Next best | Gap (pts) | Rating |");
        let _ = writeln!(w, "|---|---|---|---|---|---|---|");
        for p in &a.praise {
            let _ = writeln!(w, "| {} | {} {} | {} | {} | {} | {:.1} | {} |", p.number, p.color.name(), p.mv, p.kind.label(), p.note, if p.second.is_empty() { "—" } else { &p.second }, p.gap, p.rating.as_deref().unwrap_or("—"));
        }
        let _ = writeln!(w);
    }

    // ------------------------------------------------------- game arc facts
    let _ = writeln!(w, "## Game arc facts\n");
    let _ = writeln!(w, "Numbers for a phase-by-phase story. Winrates are Black's; \"hot regions\" are where ownership changed most during the phase (points).\n");
    if a.phases.is_empty() {
        let _ = writeln!(w, "No phase data.\n");
    } else {
        let _ = writeln!(w, "| Phase | Moves | Black winrate | Score | Student mean loss | Opponent mean loss | Student worst | Opponent worst | Student top-1 | Hot regions |");
        let _ = writeln!(w, "|---|---|---|---|---|---|---|---|---|---|");
        for ph in &a.phases {
            let worst = |x: &Option<(usize, String, f64)>| match x {
                Some((n, m, l)) => format!("{} {} ({:.1})", n, m, l),
                None => "—".to_string(),
            };
            let hot: Vec<String> = ph.hot_regions.iter().map(|(r, v)| format!("{} ({:.0})", r, v)).collect();
            let _ = writeln!(
                w,
                "| {} | {}–{} | {} → {} | {} → {} | {:.2} | {:.2} | {} | {} | {:.0}% | {} |",
                ph.name,
                ph.first_move,
                ph.last_move,
                pct(ph.winrate_start),
                pct(ph.winrate_end),
                lead(ph.score_start),
                lead(ph.score_end),
                ph.student_mean_loss,
                ph.opponent_mean_loss,
                worst(&ph.student_worst),
                worst(&ph.opponent_worst),
                ph.student_top1_rate * 100.0,
                if hot.is_empty() { "—".to_string() } else { hot.join(", ") }
            );
        }
        let _ = writeln!(w);
    }
    let _ = writeln!(w, "### Life-and-death changes\n");
    if a.status_changes.is_empty() {
        let _ = writeln!(w, "No group changed status according to the ownership maps.\n");
    } else {
        let _ = writeln!(w, "Groups whose predicted owner changed with a move (from KataGo's ownership before and after; \"captured\" = removed by the move).\n");
        let _ = writeln!(w, "| Move | Played by | Group | Stones | From | To |");
        let _ = writeln!(w, "|---|---|---|---|---|---|");
        for c in &a.status_changes {
            let _ = writeln!(w, "| {} | {} | {} {} | {} | {} | {} |", c.number, c.mover.name(), c.group_color.name(), c.anchor, c.stones, c.from, c.to);
        }
        let _ = writeln!(w);
    }

    // ------------------------------------------------------- move by move
    // Full entries: the student's teaching candidates and praised moves, the opponent's six
    // biggest mistakes, and the last move. Everything else is one line.
    let mut opp: Vec<&MoveReview> = a.reviews.iter().filter(|r| r.color != student && r.point_loss >= 2.0).collect();
    opp.sort_by(|x, y| y.point_loss.partial_cmp(&x.point_loss).unwrap());
    let key: HashSet<usize> = a
        .teaching
        .iter()
        .map(|t| t.number)
        .chain(a.praise.iter().map(|p| p.number))
        .chain(opp.iter().take(6).map(|r| r.number))
        .chain(std::iter::once(n))
        .collect();
    let _ = writeln!(w, "## Move-by-move analysis\n");
    let _ = writeln!(w, "Full entries: the student's teaching candidates and praised moves, the opponent's biggest mistakes, checkpoints and the last move. \
Every other move is one line: loss, KataGo's rank of the move, then Black's winrate and the score after it.\n");
    let mut board = Board::new(g.size_x, g.size_y);
    for c in &g.setup_black {
        board.set(*c, Some(Color::Black));
    }
    for c in &g.setup_white {
        board.set(*c, Some(Color::White));
    }
    if !g.setup_black.is_empty() || !g.setup_white.is_empty() {
        let _ = writeln!(w, "### Initial position (setup stones)\n");
        let _ = writeln!(w, "```\n{}```\n", board.render(&HashMap::new()));
    }
    if let Some(t0) = a.turns.first() {
        let _ = writeln!(
            w,
            "Before the first move KataGo evaluates the position at Black winrate {}, score {}. {} to play.\n",
            pct(t0.winrate),
            lead(t0.score_lead),
            t0.to_move.name()
        );
    }

    let checkpoint = if n > 200 { 50 } else { 25 };
    let mut in_brief_list = false;
    for (i, r) in a.reviews.iter().enumerate() {
        let m = &g.moves[i];
        if let Some(p) = m.point {
            board.play(m.color, p);
        }
        if !key.contains(&r.number) && r.number % checkpoint != 0 {
            let rank_txt = match r.rank {
                Some(0) => "#1".to_string(),
                Some(k) => format!("#{}", k + 1),
                None => "unsearched".to_string(),
            };
            let _ = writeln!(
                w,
                "- **Move {}** {} {}: loss {} ({}), then Black {} / {}.",
                r.number,
                r.color.name(),
                r.mv,
                one_dec(r.point_loss),
                rank_txt,
                pct(r.winrate_after),
                lead(r.score_after)
            );
            in_brief_list = true;
            continue;
        }
        if in_brief_list {
            let _ = writeln!(w);
            in_brief_list = false;
        }
        let deep = match a.search_coverage.requested_deep_visits {
            Some(d) => if crate::analysis::position_deep(&a.turns[r.number - 1], d) && crate::analysis::position_deep(&a.turns[r.number], d) { " ◆" } else { "" },
            None => if !a.deepened.is_empty() && a.deepened.contains(&(r.number - 1)) && a.deepened.contains(&r.number) { " ◆" } else { "" },
        };
        let _ = writeln!(w, "### Move {}: {} {}{}\n", r.number, r.color.name(), r.mv, deep);
        let rank_txt = match r.rank {
            Some(0) => "KataGo's top choice".to_string(),
            Some(k) => format!("KataGo's #{} choice", k + 1),
            None => "not searched by KataGo".to_string(),
        };
        let _ = writeln!(
            w,
            "- Evaluation: Black winrate {} → {}, score {} → {}.",
            pct(r.winrate_before),
            pct(r.winrate_after),
            lead(r.score_before),
            lead(r.score_after)
        );
        let _ = writeln!(
            w,
            "- Point loss for {}: {} pts ({}); winrate change for {}: {}%. Verdict: **{}**. Phase: {}.",
            r.color.name(),
            signed(r.point_loss),
            rank_txt,
            r.color.name(),
            signed(-r.winrate_loss * 100.0),
            r.category.label(),
            phase_of(r.number, n)
        );
        if let Some(pc) = &r.played_candidate {
            if !pc.pv.is_empty() {
                let _ = writeln!(w, "- KataGo's expected continuation after {}: {}", r.mv, pc.pv.join(" "));
            }
        }
        if let Some(best) = r.alternatives.first() {
            if best.mv != r.mv {
                let _ = writeln!(
                    w,
                    "- KataGo preferred **{}** ({}, {}), expecting: {}",
                    best.mv,
                    pct(best.winrate),
                    lead(best.score_lead),
                    if best.pv.is_empty() { "—".to_string() } else { best.pv.join(" ") }
                );
            }
        }
        if let Some(secs) = r.time_spent {
            let _ = writeln!(w, "- Time spent on this move: {:.0} s.", secs);
        }
        if let (Some(p), Some(k)) = (r.policy_prob, r.policy_rank) {
            let mut line = format!("- Network policy for {}: {} (#{} over the whole board).", r.mv, prob(p), k);
            if let (Some(hp), Some(hk)) = (r.human_prob, r.human_rank) {
                line.push_str(&format!(" Human policy: {} (#{})", prob(hp), hk));
                if let (Some(t), Some(tp)) = (&r.human_top, r.human_top_prob) {
                    line.push_str(&format!(", most common human move {} ({:.0}%)", t, tp * 100.0));
                }
                line.push('.');
            }
            if let (Some(tp), Some(tk)) = (r.target_prob, r.target_rank) {
                line.push_str(&format!(" Target profile: {} (#{})", prob(tp), tk));
                if let (Some(t), Some(pp)) = (&r.target_top, r.target_top_prob) {
                    line.push_str(&format!(", most common move {} ({:.0}%)", t, pp * 100.0));
                }
                line.push('.');
            }
            let _ = writeln!(w, "{}", line);
        }
        if let Some(c) = &r.comment {
            if !is_auto_comment(c) {
                let _ = writeln!(w, "- Comment in the game record: {}", c.replace('\n', " "));
            }
        }
        let _ = writeln!(w);
        if !r.alternatives.is_empty() {
            let best_score = r.alternatives[0].score_lead;
            let _ = writeln!(w, "Candidates in the position before this move:\n");
            let _ = writeln!(w, "{}", CAND_HEADER);
            for c in &r.alternatives {
                let _ = writeln!(w, "{}", candidate_row(c, &r.mv, best_score, r.color));
            }
            if r.rank.map_or(false, |k| k >= r.alternatives.len()) {
                if let Some(pc) = &r.played_candidate {
                    let _ = writeln!(w, "{}", candidate_row(pc, &r.mv, best_score, r.color));
                }
            }
            let _ = writeln!(w);
        }
        let show_board = r.category >= Category::Mistake || r.number % checkpoint == 0 || r.number == n;
        if show_board {
            let _ = writeln!(w, "Position after move {}:\n", r.number);
            let _ = writeln!(w, "{}", diagram(&board, r, g.size_y));
        }
    }

    if in_brief_list {
        let _ = writeln!(w);
    }

    // --------------------------------------------------------- openings + history
    if !a.openings.is_empty() {
        let _ = writeln!(w, "## Opening patterns\n");
        let _ = writeln!(w, "How each corner was played (first 50 moves), described from the stones and judged by KataGo. \
Use the summary names to research the pattern; the first deviation is the first corner move KataGo disliked by 1.5+ points.\n");
        for c in &a.openings {
            let _ = writeln!(w, "### {} corner: {}\n", c.corner, c.summary);
            for m in &c.moves {
                let _ = writeln!(
                    w,
                    "- Move {} {} {}: {} (loss {}, {})",
                    m.number,
                    m.color.name(),
                    m.mv,
                    m.description,
                    one_dec(m.point_loss),
                    match m.rank {
                        Some(0) => "KataGo's choice".to_string(),
                        Some(k) => format!("KataGo's #{}", k + 1),
                        None => "not searched".to_string(),
                    }
                );
            }
            match &c.first_deviation {
                Some((n, mv, best, loss)) => {
                    let _ = writeln!(w, "\nFirst deviation: move {} {} (KataGo preferred {}, {:.1} points).\n", n, mv, best, loss);
                }
                None => {
                    let _ = writeln!(w, "\nNo move in this corner lost 1.5 points or more.\n");
                }
            }
        }
    }
    if a.history.len() >= 2 {
        let _ = writeln!(w, "## Compared with the student's earlier games\n");
        let me = crate::progress::entry_for(a);
        let recent: Vec<&crate::progress::GameEntry> = a.history.iter().rev().take(10).collect();
        let avg = |f: &dyn Fn(&crate::progress::GameEntry) -> f64| recent.iter().map(|e| f(e)).sum::<f64>() / recent.len() as f64;
        let _ = writeln!(w, "The student has {} earlier analysed games; averages below are over the last {}.\n", a.history.len(), recent.len());
        let _ = writeln!(w, "| Metric | This game | Recent average |");
        let _ = writeln!(w, "|---|---|---|");
        let _ = writeln!(w, "| Mean point loss per move | {:.2} | {:.2} |", me.mean_loss, avg(&|e| e.mean_loss));
        let _ = writeln!(w, "| Opening mean loss | {:.2} | {:.2} |", me.opening_loss, avg(&|e| e.opening_loss));
        let _ = writeln!(w, "| Middlegame mean loss | {:.2} | {:.2} |", me.middlegame_loss, avg(&|e| e.middlegame_loss));
        let _ = writeln!(w, "| Endgame mean loss | {:.2} | {:.2} |", me.endgame_loss, avg(&|e| e.endgame_loss));
        let _ = writeln!(w, "| Matched KataGo's top choice | {:.0}% | {:.0}% |", me.top1_rate * 100.0, avg(&|e| e.top1_rate) * 100.0);
        let _ = writeln!(w, "| Moves losing 3+ points | {} | {:.1} |", me.mistakes, avg(&|e| e.mistakes as f64));
        let _ = writeln!(w);
        let mut theme_totals: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
        for e in &recent {
            for (k, v) in &e.themes {
                *theme_totals.entry(k.clone()).or_insert(0) += v;
            }
        }
        if !theme_totals.is_empty() {
            let mut tv: Vec<(String, usize)> = theme_totals.into_iter().collect();
            tv.sort_by(|x, y| y.1.cmp(&x.1));
            let list: Vec<String> = tv.iter().take(4).map(|(k, v)| format!("{} ({})", k, v)).collect();
            let _ = writeln!(w, "Recurring themes in recent games (teaching candidates per theme): {}.\n", list.join(", "));
        }
        let _ = writeln!(w, "| Game | Date | Opponent | Result | Mean loss | Top-1 |");
        let _ = writeln!(w, "|---|---|---|---|---|---|");
        for e in recent.iter().rev() {
            let _ = writeln!(
                w,
                "| {} | {} | {} | {} | {:.2} | {:.0}% |",
                if e.file_stem.is_empty() { e.analysed_at.chars().take(10).collect::<String>() } else { e.file_stem.clone() },
                e.date.as_deref().unwrap_or("—"),
                e.opponent_name.as_deref().unwrap_or("—"),
                e.result.as_deref().unwrap_or("—"),
                e.mean_loss,
                e.top1_rate * 100.0
            );
        }
        let _ = writeln!(w);
    }

    // --------------------------------------------------------- final position
    if let Some(last) = a.turns.last() {
        let _ = writeln!(w, "## Final position\n");
        let _ = writeln!(
            w,
            "After move {} KataGo estimates Black winrate {} and score {}. {} would be next to play.\n",
            n,
            pct(last.winrate),
            lead(last.score_lead),
            last.to_move.name()
        );
        if n == 0 {
            let _ = writeln!(w, "```\n{}```\n", board.render(&HashMap::new()));
        }
        if !last.candidates.is_empty() {
            let _ = writeln!(w, "KataGo's suggestions for the next move:\n");
            let _ = writeln!(w, "{}", CAND_HEADER);
            let best_score = last.candidates[0].score_lead;
            for c in last.candidates.iter().take(a.options.top_moves) {
                let _ = writeln!(w, "{}", candidate_row(c, "", best_score, last.to_move));
            }
            let _ = writeln!(w);
        }
    }

    // ------------------------------------------------------------- appendix
    let _ = writeln!(w, "## Appendix: compact move list\n");
    let _ = writeln!(w, "One line per move: number, colour, move, point loss, rank, Black winrate after the move, score after the move.\n");
    let _ = writeln!(w, "```");
    for r in &a.reviews {
        let _ = writeln!(
            w,
            "{:>3} {} {:<5} loss {:>5}  rank {:<4} wr {:>6}  {}",
            r.number,
            r.color.letter(),
            r.mv,
            one_dec(r.point_loss),
            r.rank.map(|k| format!("#{}", k + 1)).unwrap_or_else(|| "-".into()),
            pct(r.winrate_after),
            lead(r.score_after)
        );
    }
    let _ = writeln!(w, "```");

    out
}


/// The agent-facing report carries one authoritative data block. Detailed per-move prose is
/// still available through render_detailed_markdown and the app's detailed-report download.
pub fn render_markdown(a: &GameAnalysis) -> String {
    let mut out = String::new();
    let student = a.student.unwrap_or(Color::Black);
    let _ = writeln!(out, "# Go Teacher — teaching evidence\n\nReport format: {}\n", REPORT_FORMAT);
    let _ = writeln!(out, "{} vs {} · {}×{} · student {} · {} moves.\n", opt(&a.game.player_black), opt(&a.game.player_white), a.game.size_x, a.game.size_y, student.name(), a.game.moves.len());
    let _ = writeln!(out, "{}; {}. Teaching searches: {} queries in {:.0}s.\n", a.engine_version, a.visits_setting, a.probes.queries, a.probes.elapsed_seconds);
    out.push_str("Scores are always Black's perspective (B+ / W+). Positive losses are costs to the named mover. Winrate saturation is not a prediction that a human game cannot turn around. Pass comparisons are whole-board estimates. Ownership and restricted searches are evidence, not life/death proofs. Human continuations are sampled illustrations; their endpoint differences include later choices. Profile fit is not a calibrated rank.\n\n");
    out.push_str("## Teaching candidates\n\n| Move | Student move | Better move | Loss for mover | Theme | Deep evaluation | Teaching investigations |\n|---|---|---|---|---|---|---|\n");
    for t in &a.teaching {
        let cov = crate::analysis::candidate_coverage(&a.turns, t.number, a.search_coverage.requested_deep_visits, a.search_coverage.two_pass_enabled);
        let deep_txt = match cov.status.as_str() {
            "complete" => format!("Complete · {} / {} visits", cov.before.visits.min(cov.after.visits), cov.before.requested_visits.unwrap_or(0)),
            "incomplete" => format!("Incomplete · before {}, after {}", cov.before.status, cov.after.status),
            "disabled" => format!("Two-pass disabled · {} visits", cov.before.visits.min(cov.after.visits)),
            _ => "Coverage not recorded".to_string(),
        };
        let inv_txt = match a.probes.moments.iter().find(|p| p.move_number == t.number) {
            Some(p) => match crate::probes::investigation_status(p)["status"].as_str().unwrap_or("") { "available" => "Available", "partial" => "Partial", _ => "Unavailable" }.to_string(),
            None => if a.options.probes.enabled { "Not selected".to_string() } else { "Disabled".to_string() },
        };
        let _ = writeln!(out, "| {} | {} {} | {} | {:.1} pts | {} | {} | {} |", t.number, t.color.name(), t.mv, t.best.as_deref().unwrap_or("unknown"), t.point_loss, t.theme.label(), deep_txt, inv_txt);
    }
    let c = &a.search_coverage;
    if c.two_pass_enabled {
        let _ = writeln!(out, "\nDeep evaluation: {} of {} candidates verified on both surrounding positions at {} requested visits; {} verification round{} added {} position{} beyond the initial deep pass. \"Not selected\" in the last column means the optional investigations (variation tree, local reading, human lines) were not run for that move; its deep evaluation, better line and refutation are unaffected.", c.verified_candidates, c.final_candidates, c.requested_deep_visits.unwrap_or(0), c.verification_rounds, if c.verification_rounds == 1 { "" } else { "s" }, c.additional_positions, if c.additional_positions == 1 { "" } else { "s" });
    } else {
        out.push_str("\nSingle-pass run: every position was searched once at the configured budget; candidate verification was not requested.\n");
    }
    // Student profile
    out.push_str("\n## Student profile\n\n");
    match a.probes.rank_fit.get("estimate").and_then(|e| e.get("wording")).and_then(|w| w.as_str()) {
        Some(w) => {
            let _ = writeln!(out, "The student {}.", w);
            if let Some(phases) = a.probes.rank_fit.get("phases").and_then(|p| p.as_array()) {
                let parts: Vec<String> = phases
                    .iter()
                    .filter(|f| f["phase"] != "whole game")
                    .filter_map(|f| f.get("estimate").and_then(|e| e.get("rank_label")).and_then(|l| l.as_str()).map(|l| format!("{} {}", f["phase"].as_str().unwrap_or("?"), l)))
                    .collect();
                if !parts.is_empty() {
                    let _ = writeln!(out, "By phase: {}.", parts.join(", "));
                }
            }
        }
        None => out.push_str("No rank estimate for this game (human model not loaded or too few student moves).\n"),
    }
    if let Some((v, label, n)) = crate::progress::working_rank(&a.history, a.probes.rank_fit.get("estimate").and_then(|e| e.get("rank_value")).and_then(|x| x.as_f64())) {
        let _ = writeln!(out, "Working rank across the last {} analysed game{}: about {} (value {:.1}).", n, if n == 1 { "" } else { "s" }, label, v);
    }
    let _ = writeln!(out, "Human profiles used by the deeper searches: {}.", a.probes.profiles);
    out.push_str("This is similarity to human play at the tested ranks in one game, not a rating; say \"plays like\", pick the level of explanation and puzzles from it.\n");

    // Decided-game framing
    {
        let mut from: Option<usize> = None;
        for t in a.turns.iter() {
            if (0.05..=0.95).contains(&t.winrate) { from = None; } else if from.is_none() { from = Some(t.turn); }
        }
        if let Some(f) = from {
            if a.turns.len() > f + 20 {
                let _ = writeln!(out, "\nGame framing: KataGo rated the game as decided from position {} onward (final estimate {}). Losses after that point changed the margin, not the result; judge them by points and take the main lessons from the moves up to position {}.", f, a.turns.last().map(|t| lead(t.score_lead)).unwrap_or_default(), f);
            }
        }
    }

    // Good moves
    out.push_str("\n## Good moves\n\n");
    if a.praise.is_empty() {
        let captures = a.reviews.iter().filter(|r| r.color == student && r.stones_captured > 0).count();
        if captures > 0 {
            let _ = writeln!(out, "No move met the praise thresholds, but the student captured stones on {} move(s); see stones_captured in the move list.\n", captures);
        } else {
            out.push_str("No move met the praise thresholds in this game.\n");
        }
    } else {
        out.push_str("| Move | Played | Why it is praised | Gap to next best (pts) | Stones captured | Rating |\n|---|---|---|---|---|---|\n");
        for p in &a.praise {
            let gap_txt = if p.second.is_empty() { "—".to_string() } else if p.gap >= 0.0 { format!("{:.1}", p.gap) } else { format!("{:.1} (another searched move scored higher)", p.gap) };
            let _ = writeln!(out, "| {} | {} {} | {} — {} | {} | {} | {} |", p.number, p.color.name(), p.mv, p.kind.label(), p.note, gap_txt, p.stones_captured, p.rating.as_deref().unwrap_or("—"));
        }
        out.push_str("\nRatings come from the human-style network: the weakest tested rank whose players choose the move first. Quote them with that source.\n");
    }

    out.push_str("\n## How to use this report\n\nParse the block below with the go-game-teacher skill. It contains the complete move list, setup stones, game arc facts, praise, teaching evidence and sequence origins. The skill writes prose and references evidence by move number; its generator loads numerical facts directly. Large heatmap arrays remain in the application's JSON. The detailed per-move Markdown report remains available in the app.\n\n");
    out.push_str("The move list also records each move's point loss and resulting numeric Black score lead, with the initial evaluation recorded once. Use this timeline and the phase statistics for a short overview; keep the lesson focused on selected mistakes, replies and better choices. Timing is included only where recorded. Full variations are limited to teaching candidates, praise, the opponent’s biggest mistakes, checkpoints and the last move.\n\n");
    for p in &a.probes.moments {
        let _ = writeln!(out, "### Move {}\n", p.move_number);
        if let Some(value) = p.pass_comparison["best_vs_pass_points"].as_f64() {
            let _ = writeln!(out, "Best play versus passing: {:+.1} points for {}.\n", value, p.mover.name());
        }
        for v in p.initiative.iter().take(2) { let _=writeln!(out,"- {}: {} (estimate).",v["move"].as_str().unwrap_or("?"),v["class"].as_str().unwrap_or("unclear")); }
        if !p.unavailable.is_empty() { let _ = writeln!(out, "- Unavailable: {}.", p.unavailable.iter().map(|(k,v)|format!("{k}: {v}")).collect::<Vec<_>>().join("; ")); }
        out.push('\n');
    }
    out.push_str("## Evidence data\n\n```go-teacher-evidence\n");
    out.push_str(&serde_json::to_string(&crate::evidence::build(a)).expect("evidence is serializable"));
    out.push_str("\n```\n");
    out
}
