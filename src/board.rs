//! Board state tracking (captures only, no legality checks) and ASCII diagrams.

use crate::sgf::{Color, Coord};
use std::collections::HashMap;

#[derive(Clone)]
pub struct Board {
    pub size_x: usize,
    pub size_y: usize,
    cells: Vec<Option<Color>>,
}

impl Board {
    pub fn new(size_x: usize, size_y: usize) -> Board {
        Board {
            size_x,
            size_y,
            cells: vec![None; size_x * size_y],
        }
    }

    fn idx(&self, c: Coord) -> usize {
        c.y * self.size_x + c.x
    }

    pub fn get(&self, c: Coord) -> Option<Color> {
        self.cells[self.idx(c)]
    }

    pub fn set(&mut self, c: Coord, v: Option<Color>) {
        let i = self.idx(c);
        self.cells[i] = v;
    }

    fn neighbors(&self, c: Coord) -> Vec<Coord> {
        let mut n = Vec::with_capacity(4);
        if c.x > 0 {
            n.push(Coord { x: c.x - 1, y: c.y });
        }
        if c.x + 1 < self.size_x {
            n.push(Coord { x: c.x + 1, y: c.y });
        }
        if c.y > 0 {
            n.push(Coord { x: c.x, y: c.y - 1 });
        }
        if c.y + 1 < self.size_y {
            n.push(Coord { x: c.x, y: c.y + 1 });
        }
        n
    }

    /// Returns the group containing `start` and whether it has any liberty.
    fn group(&self, start: Coord) -> (Vec<Coord>, bool) {
        let color = match self.get(start) {
            Some(c) => c,
            None => return (vec![], true),
        };
        let mut stack = vec![start];
        let mut seen = vec![false; self.cells.len()];
        seen[self.idx(start)] = true;
        let mut group = Vec::new();
        let mut has_liberty = false;
        while let Some(c) = stack.pop() {
            group.push(c);
            for n in self.neighbors(c) {
                match self.get(n) {
                    None => has_liberty = true,
                    Some(col) if col == color => {
                        let i = self.idx(n);
                        if !seen[i] {
                            seen[i] = true;
                            stack.push(n);
                        }
                    }
                    _ => {}
                }
            }
        }
        (group, has_liberty)
    }

    /// Place a stone and resolve captures. Returns the number of stones captured.
    pub fn play(&mut self, color: Color, point: Coord) -> usize {
        self.set(point, Some(color));
        let mut captured = 0;
        for n in self.neighbors(point) {
            if self.get(n) == Some(color.opponent()) {
                let (g, lib) = self.group(n);
                if !lib {
                    captured += g.len();
                    for c in g {
                        self.set(c, None);
                    }
                }
            }
        }
        // Suicide (legal under some rule sets): remove own group if it has no liberties.
        let (g, lib) = self.group(point);
        if !lib {
            for c in g {
                self.set(c, None);
            }
        }
        captured
    }

    /// Every group on the board with its colour, stones and liberty count.
    pub fn groups(&self) -> Vec<(Color, Vec<Coord>, usize)> {
        let mut seen = vec![false; self.cells.len()];
        let mut out = Vec::new();
        for y in 0..self.size_y {
            for x in 0..self.size_x {
                let c = Coord { x, y };
                let i = self.idx(c);
                if seen[i] {
                    continue;
                }
                let Some(color) = self.get(c) else { continue };
                seen[i] = true;
                let mut stack = vec![c];
                let mut group = Vec::new();
                let mut libs = std::collections::HashSet::new();
                while let Some(p) = stack.pop() {
                    group.push(p);
                    for n in self.neighbors(p) {
                        match self.get(n) {
                            None => {
                                libs.insert(n);
                            }
                            Some(col) if col == color => {
                                let j = self.idx(n);
                                if !seen[j] {
                                    seen[j] = true;
                                    stack.push(n);
                                }
                            }
                            _ => {}
                        }
                    }
                }
                out.push((color, group, libs.len()));
            }
        }
        out
    }

    pub fn stones(&self, color: Color) -> Vec<Coord> {
        let mut v = Vec::new();
        for y in 0..self.size_y {
            for x in 0..self.size_x {
                let c = Coord { x, y };
                if self.get(c) == Some(color) {
                    v.push(c);
                }
            }
        }
        v
    }

    fn is_star(&self, x: usize, y: usize) -> bool {
        let stars = |n: usize| -> Vec<usize> {
            match n {
                19 => vec![3, 9, 15],
                13 => vec![3, 6, 9],
                9 => vec![2, 4, 6],
                _ => vec![],
            }
        };
        stars(self.size_x).contains(&x) && stars(self.size_y).contains(&y)
    }

    /// Render an ASCII diagram. `marks` maps coordinates to a single display character that
    /// overrides the stone/empty glyph (used for the last move and AI suggestions).
    pub fn render(&self, marks: &HashMap<Coord, char>) -> String {
        const COLS: &[u8] = b"ABCDEFGHJKLMNOPQRSTUVWXYZ";
        let mut out = String::new();
        let header: String = (0..self.size_x)
            .map(|x| format!("{} ", COLS[x] as char))
            .collect();
        out.push_str(&format!("    {}\n", header.trim_end()));
        for y in 0..self.size_y {
            let row = self.size_y - y;
            out.push_str(&format!("{:>2}  ", row));
            for x in 0..self.size_x {
                let c = Coord { x, y };
                let ch = if let Some(m) = marks.get(&c) {
                    *m
                } else {
                    match self.get(c) {
                        Some(Color::Black) => 'X',
                        Some(Color::White) => 'O',
                        None => {
                            if self.is_star(x, y) {
                                '+'
                            } else {
                                '.'
                            }
                        }
                    }
                };
                out.push(ch);
                if x + 1 < self.size_x {
                    out.push(' ');
                }
            }
            out.push_str(&format!("  {:<2}\n", row));
        }
        out.push_str(&format!("    {}\n", header.trim_end()));
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn captures_single_stone() {
        let mut b = Board::new(9, 9);
        let w = Coord { x: 4, y: 4 };
        b.play(Color::White, w);
        b.play(Color::Black, Coord { x: 3, y: 4 });
        b.play(Color::Black, Coord { x: 5, y: 4 });
        b.play(Color::Black, Coord { x: 4, y: 3 });
        let captured = b.play(Color::Black, Coord { x: 4, y: 5 });
        assert_eq!(captured, 1);
        assert_eq!(b.get(w), None);
    }
}

/// Replay guard for displayed and sampled lines. The engine remains authoritative for rules
/// details (e.g. Japanese encore); this rejects malformed, occupied, suicide and ko moves.
#[derive(Clone)]
pub struct LegalPosition {
    pub board: Board,
    history: Vec<(Vec<Option<Color>>, Color)>,
    pub to_move: Color,
    rules: String,
}

impl LegalPosition {
    pub fn from_game(game: &crate::sgf::GameRecord, turn: usize, rules: &str) -> anyhow::Result<Self> {
        let mut board = Board::new(game.size_x, game.size_y);
        for (color, stones) in [(Color::Black, &game.setup_black), (Color::White, &game.setup_white)] {
            for &c in stones {
                if c.x >= game.size_x || c.y >= game.size_y || board.get(c).is_some() {
                    anyhow::bail!("invalid setup stone");
                }
                board.set(c, Some(color));
            }
        }
        let mut p = Self { history: vec![(board.cells.clone(), game.who_moves_first())], board, to_move: game.who_moves_first(), rules: rules.to_string() };
        for m in game.moves.iter().take(turn) {
            // Records may explicitly contain non-alternating moves (e.g. teaching records).
            p.to_move = m.color;
            p.play(&m.point.map(|c| c.to_gtp(game.size_y)).unwrap_or_else(|| "pass".into()))?;
        }
        Ok(p)
    }

    pub fn play(&mut self, mv: &str) -> anyhow::Result<()> {
        let next = self.to_move.opponent();
        if mv.eq_ignore_ascii_case("pass") {
            self.to_move = next;
            self.history.push((self.board.cells.clone(), next));
            return Ok(());
        }
        let c = Coord::from_gtp(mv, self.board.size_y).ok_or_else(|| anyhow::anyhow!("invalid coordinate {mv}"))?;
        if c.x >= self.board.size_x || self.board.get(c).is_some() { anyhow::bail!("occupied or out-of-board move {mv}"); }
        let mut b = self.board.clone();
        b.play(self.to_move, c);
        if b.get(c).is_none() && !matches!(self.rules.as_str(), "new-zealand" | "tromp-taylor") {
            anyhow::bail!("suicide at {mv}");
        }
        let repeat = if matches!(self.rules.as_str(), "japanese" | "korean") {
            self.history.len() >= 2 && self.history[self.history.len()-2].0 == b.cells
        } else if matches!(self.rules.as_str(), "aga" | "bga" | "chinese-kgs") {
            self.history.iter().any(|(cells, player)| *cells == b.cells && *player == next)
        } else {
            self.history.iter().any(|(cells, _)| *cells == b.cells)
        };
        if repeat { anyhow::bail!("ko repetition at {mv}"); }
        self.board = b;
        self.to_move = next;
        self.history.push((self.board.cells.clone(), next));
        Ok(())
    }
}

#[cfg(test)]
mod replay_tests {
    use super::*;
    #[test]
    fn setup_ko_capture_and_pass() {
        let game=crate::sgf::parse_game("(;SZ[9]AB[ah][bi][ch]AW[bh][ag][cg][bf]PL[B])").unwrap();
        let mut p=LegalPosition::from_game(&game,0,"japanese").unwrap();
        p.play("B3").unwrap();
        assert!(p.play("B2").is_err());
        assert_eq!(p.to_move,Color::White);
        p.play("pass").unwrap();p.play("H9").unwrap();p.play("B2").unwrap();
        assert_eq!(p.board.get(Coord::from_gtp("B3",9).unwrap()),None);
        assert!(p.play("I3").is_err());assert!(p.play("T3").is_err());
    }
}
