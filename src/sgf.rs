//! Minimal SGF (Smart Game Format) parser tailored to what a game review needs:
//! root metadata, setup stones, and the main line of moves.

use anyhow::{anyhow, bail, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Color {
    Black,
    White,
}

impl Color {
    pub fn opponent(self) -> Color {
        match self {
            Color::Black => Color::White,
            Color::White => Color::Black,
        }
    }
    pub fn letter(self) -> &'static str {
        match self {
            Color::Black => "B",
            Color::White => "W",
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Color::Black => "Black",
            Color::White => "White",
        }
    }
}

/// Board coordinate: `x` is the column from the left (0-based), `y` the row from the top (0-based).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct Coord {
    pub x: usize,
    pub y: usize,
}

const GTP_COLS: &[u8] = b"ABCDEFGHJKLMNOPQRSTUVWXYZ";

impl Coord {
    /// GTP / KataGo style coordinate such as `D4` or `Q16` (no letter `I`).
    pub fn to_gtp(&self, size_y: usize) -> String {
        let col = GTP_COLS[self.x] as char;
        format!("{}{}", col, size_y - self.y)
    }

    pub fn from_gtp(s: &str, size_y: usize) -> Option<Coord> {
        let s = s.trim().to_ascii_uppercase();
        let mut chars = s.chars();
        let c = chars.next()?;
        let x = GTP_COLS.iter().position(|&b| b as char == c)?;
        let row: usize = chars.as_str().parse().ok()?;
        if row == 0 || row > size_y {
            return None;
        }
        Some(Coord { x, y: size_y - row })
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Move {
    pub color: Color,
    /// `None` means a pass.
    pub point: Option<Coord>,
    /// Comment attached to this move in the SGF, if any.
    pub comment: Option<String>,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct GameRecord {
    pub size_x: usize,
    pub size_y: usize,
    pub komi: Option<f64>,
    pub rules: Option<String>,
    pub handicap: Option<u32>,
    pub setup_black: Vec<Coord>,
    pub setup_white: Vec<Coord>,
    /// Explicit player-to-move from the `PL` property, if given.
    pub first_player: Option<Color>,
    pub moves: Vec<Move>,
    pub player_black: Option<String>,
    pub player_white: Option<String>,
    pub rank_black: Option<String>,
    pub rank_white: Option<String>,
    pub result: Option<String>,
    pub date: Option<String>,
    pub event: Option<String>,
    pub game_name: Option<String>,
    pub place: Option<String>,
    pub time_settings: Option<String>,
    pub overtime: Option<String>,
    pub game_comment: Option<String>,
    pub application: Option<String>,
    /// Non-fatal oddities found while reading the file.
    pub warnings: Vec<String>,
}

impl GameRecord {
    pub fn who_moves_first(&self) -> Color {
        if let Some(c) = self.first_player {
            return c;
        }
        if let Some(m) = self.moves.first() {
            return m.color;
        }
        if !self.setup_black.is_empty() && self.setup_white.is_empty() {
            return Color::White;
        }
        Color::Black
    }
}

// ---------------------------------------------------------------------------
// Raw tree
// ---------------------------------------------------------------------------

#[derive(Debug, Default)]
pub struct Node {
    pub props: Vec<(String, Vec<String>)>,
}

impl Node {
    pub fn get(&self, key: &str) -> Option<&Vec<String>> {
        self.props.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }
    pub fn first(&self, key: &str) -> Option<&str> {
        self.get(key).and_then(|v| v.first()).map(|s| s.as_str())
    }
}

#[derive(Debug, Default)]
pub struct Tree {
    pub nodes: Vec<Node>,
    pub children: Vec<Tree>,
}

struct Parser {
    chars: Vec<char>,
    pos: usize,
}

impl Parser {
    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }
    fn skip_ws(&mut self) {
        while matches!(self.peek(), Some(c) if c.is_whitespace()) {
            self.pos += 1;
        }
    }
    fn expect(&mut self, c: char) -> Result<()> {
        self.skip_ws();
        match self.peek() {
            Some(x) if x == c => {
                self.pos += 1;
                Ok(())
            }
            other => bail!("SGF parse error at offset {}: expected '{}', found {:?}", self.pos, c, other),
        }
    }

    fn parse_tree(&mut self) -> Result<Tree> {
        self.expect('(')?;
        let mut tree = Tree::default();
        loop {
            self.skip_ws();
            match self.peek() {
                Some(';') => tree.nodes.push(self.parse_node()?),
                Some('(') => tree.children.push(self.parse_tree()?),
                Some(')') => {
                    self.pos += 1;
                    break;
                }
                None => bail!("SGF parse error: unexpected end of file inside a game tree"),
                Some(c) => bail!("SGF parse error at offset {}: unexpected character {:?}", self.pos, c),
            }
        }
        Ok(tree)
    }

    fn parse_node(&mut self) -> Result<Node> {
        self.expect(';')?;
        let mut node = Node::default();
        loop {
            self.skip_ws();
            match self.peek() {
                Some(c) if c.is_ascii_alphabetic() => {
                    let mut ident = String::new();
                    while let Some(c) = self.peek() {
                        if c.is_ascii_alphabetic() {
                            // FF[3] allowed lowercase letters in identifiers; they are ignored.
                            if c.is_ascii_uppercase() {
                                ident.push(c);
                            }
                            self.pos += 1;
                        } else {
                            break;
                        }
                    }
                    let mut values = Vec::new();
                    loop {
                        self.skip_ws();
                        if self.peek() == Some('[') {
                            values.push(self.parse_value()?);
                        } else {
                            break;
                        }
                    }
                    if values.is_empty() {
                        bail!("SGF parse error at offset {}: property {} has no value", self.pos, ident);
                    }
                    node.props.push((ident, values));
                }
                _ => break,
            }
        }
        Ok(node)
    }

    fn parse_value(&mut self) -> Result<String> {
        self.expect('[')?;
        let mut out = String::new();
        loop {
            match self.peek() {
                None => bail!("SGF parse error: unterminated property value"),
                Some(']') => {
                    self.pos += 1;
                    break;
                }
                Some('\\') => {
                    self.pos += 1;
                    match self.peek() {
                        Some('\n') | Some('\r') => {
                            // Soft line break: removed entirely.
                            self.pos += 1;
                            if self.peek() == Some('\n') {
                                self.pos += 1;
                            }
                        }
                        Some(c) => {
                            out.push(c);
                            self.pos += 1;
                        }
                        None => bail!("SGF parse error: dangling escape"),
                    }
                }
                Some(c) => {
                    out.push(c);
                    self.pos += 1;
                }
            }
        }
        Ok(out)
    }
}

pub fn parse_tree(input: &str) -> Result<Tree> {
    let mut p = Parser {
        chars: input.chars().collect(),
        pos: 0,
    };
    // Tolerate junk (BOM, whitespace, HTML fragments) before the first '('.
    while p.peek().map_or(false, |c| c != '(') {
        p.pos += 1;
    }
    if p.peek().is_none() {
        bail!("Not an SGF file: no game tree found");
    }
    p.parse_tree()
}

fn is_move_node(n: &Node) -> bool {
    n.get("B").is_some() || n.get("W").is_some()
}

fn ends_with_pass(nodes: &[Node]) -> bool {
    nodes
        .iter()
        .rev()
        .find(|n| is_move_node(n))
        .map(|n| {
            let v = n.first("B").or_else(|| n.first("W")).unwrap_or("").trim();
            v.is_empty() || v == "tt"
        })
        .unwrap_or(false)
}

/// (number of moves on the best line through this subtree, whether that line ends with a pass).
/// Later siblings win ties: game records (KaTrain, OGS) append the actual continuation after
/// abandoned try-outs.
fn best_line(t: &Tree) -> (usize, bool) {
    let here = t.nodes.iter().filter(|n| is_move_node(n)).count();
    match best_child(t) {
        Some(c) => {
            let (m, p) = best_line(c);
            (here + m, if m > 0 { p } else { ends_with_pass(&t.nodes) })
        }
        None => (here, ends_with_pass(&t.nodes)),
    }
}

fn best_child(t: &Tree) -> Option<&Tree> {
    let mut best: Option<(&Tree, (usize, bool))> = None;
    for c in &t.children {
        let score = best_line(c);
        // `>=` so that a later sibling with an equal score replaces an earlier one.
        if best.map_or(true, |(_, b)| score >= b) {
            best = Some((c, score));
        }
    }
    best.map(|(c, _)| c)
}

/// Number of distinct lines (leaves) in the tree.
pub fn count_lines(t: &Tree) -> usize {
    if t.children.is_empty() {
        1
    } else {
        t.children.iter().map(count_lines).sum()
    }
}

/// The line to analyse: at every branch point follow the variation with the most moves, preferring
/// (on ties) a line that ends with passes, then the variation added last.
pub fn main_line(tree: &Tree) -> Vec<&Node> {
    let mut out = Vec::new();
    let mut t = tree;
    loop {
        out.extend(t.nodes.iter());
        match best_child(t) {
            Some(child) => t = child,
            None => break,
        }
    }
    out
}

/// The line a naive reader would take: always the first variation.
fn first_line_moves(tree: &Tree) -> usize {
    let mut n = 0;
    let mut t = tree;
    loop {
        n += t.nodes.iter().filter(|x| is_move_node(x)).count();
        match t.children.first() {
            Some(c) => t = c,
            None => return n,
        }
    }
}

fn parse_point(value: &str, size_x: usize, size_y: usize) -> Result<Option<Coord>> {
    let v = value.trim();
    if v.is_empty() {
        return Ok(None);
    }
    if v == "tt" && size_x <= 19 && size_y <= 19 {
        return Ok(None);
    }
    let bytes = v.as_bytes();
    if bytes.len() != 2 {
        bail!("bad SGF coordinate {:?}", value);
    }
    let idx = |b: u8| -> Result<usize> {
        match b {
            b'a'..=b'z' => Ok((b - b'a') as usize),
            b'A'..=b'Z' => Ok((b - b'A') as usize + 26),
            _ => bail!("bad SGF coordinate {:?}", value),
        }
    };
    let (x, y) = (idx(bytes[0])?, idx(bytes[1])?);
    if x >= size_x || y >= size_y {
        bail!("SGF coordinate {:?} is off the {}x{} board", value, size_x, size_y);
    }
    Ok(Some(Coord { x, y }))
}

fn clean(s: &str) -> Option<String> {
    let t = s.trim();
    if t.is_empty() {
        None
    } else {
        Some(t.to_string())
    }
}

/// Parse SGF text into a [`GameRecord`] following the main line only.
pub fn parse_game(input: &str) -> Result<GameRecord> {
    let tree = parse_tree(input)?;
    let nodes = main_line(&tree);
    let root = nodes.first().ok_or_else(|| anyhow!("SGF file has no nodes"))?;

    let mut game = GameRecord::default();

    // Board size: "19" or "19:13".
    let (sx, sy) = match root.first("SZ") {
        Some(sz) => {
            let sz = sz.trim();
            if let Some((a, b)) = sz.split_once(':') {
                (a.trim().parse::<usize>()?, b.trim().parse::<usize>()?)
            } else {
                let n = sz.parse::<usize>()?;
                (n, n)
            }
        }
        None => (19, 19),
    };
    if !(2..=25).contains(&sx) || !(2..=25).contains(&sy) {
        bail!("unsupported board size {}x{}", sx, sy);
    }
    game.size_x = sx;
    game.size_y = sy;

    game.komi = root.first("KM").and_then(|k| k.trim().parse::<f64>().ok());
    game.rules = root.first("RU").and_then(clean);
    game.handicap = root.first("HA").and_then(|h| h.trim().parse::<u32>().ok());
    game.player_black = root.first("PB").and_then(clean);
    game.player_white = root.first("PW").and_then(clean);
    game.rank_black = root.first("BR").and_then(clean);
    game.rank_white = root.first("WR").and_then(clean);
    game.result = root.first("RE").and_then(clean);
    game.date = root.first("DT").and_then(clean);
    game.event = root.first("EV").and_then(clean);
    game.game_name = root.first("GN").and_then(clean);
    game.place = root.first("PC").and_then(clean);
    game.time_settings = root.first("TM").and_then(clean);
    game.overtime = root.first("OT").and_then(clean);
    game.game_comment = root.first("GC").and_then(clean);
    game.application = root.first("AP").and_then(clean);
    game.first_player = match root.first("PL").map(|s| s.trim().to_ascii_uppercase()) {
        Some(ref s) if s == "B" || s == "1" => Some(Color::Black),
        Some(ref s) if s == "W" || s == "2" => Some(Color::White),
        _ => None,
    };

    let mut seen_move = false;
    for (i, node) in nodes.iter().enumerate() {
        // Setup stones are only representable before the first move.
        for (key, list) in [("AB", &mut game.setup_black), ("AW", &mut game.setup_white)] {
            if let Some(vals) = node.get(key) {
                if seen_move {
                    game.warnings.push(format!(
                        "node {} places setup stones ({}) after moves have been played; they were ignored",
                        i, key
                    ));
                    continue;
                }
                for v in vals {
                    // Compressed point lists "aa:cc" are allowed in FF[4].
                    if let Some((a, b)) = v.split_once(':') {
                        let p1 = parse_point(a, sx, sy)?.ok_or_else(|| anyhow!("bad point list"))?;
                        let p2 = parse_point(b, sx, sy)?.ok_or_else(|| anyhow!("bad point list"))?;
                        for y in p1.y.min(p2.y)..=p1.y.max(p2.y) {
                            for x in p1.x.min(p2.x)..=p1.x.max(p2.x) {
                                list.push(Coord { x, y });
                            }
                        }
                    } else if let Some(p) = parse_point(v, sx, sy)? {
                        list.push(p);
                    }
                }
            }
        }
        if node.get("AE").is_some() {
            game.warnings.push(format!("node {} removes stones (AE), which is not supported; ignored", i));
        }

        let comment = node.first("C").and_then(clean);
        let mut played = false;
        for (key, color) in [("B", Color::Black), ("W", Color::White)] {
            if let Some(v) = node.first(key) {
                if played {
                    game.warnings.push(format!("node {} contains both a Black and a White move; the second was ignored", i));
                    continue;
                }
                let point = parse_point(v, sx, sy)?;
                game.moves.push(Move {
                    color,
                    point,
                    comment: comment.clone(),
                });
                played = true;
                seen_move = true;
            }
        }
    }

    let lines = count_lines(&tree);
    if lines > 1 {
        game.warnings.push(format!(
            "the SGF contains {} variations; analysed the longest line ({} moves; the first variation has {})",
            lines,
            game.moves.len(),
            first_line_moves(&tree)
        ));
    }
    if game.moves.is_empty() && game.setup_black.is_empty() && game.setup_white.is_empty() {
        game.warnings.push("the game contains no moves".to_string());
    }

    Ok(game)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_basic_game() {
        let sgf = "(;GM[1]FF[4]SZ[19]KM[7.5]RU[Chinese]PB[Alice]PW[Bob]RE[B+R];B[pd]C[hello];W[dp];B[tt];W[])";
        let g = parse_game(sgf).unwrap();
        assert_eq!(g.size_x, 19);
        assert_eq!(g.komi, Some(7.5));
        assert_eq!(g.moves.len(), 4);
        assert_eq!(g.moves[0].point.unwrap().to_gtp(19), "Q16");
        assert_eq!(g.moves[0].comment.as_deref(), Some("hello"));
        assert_eq!(g.moves[1].point.unwrap().to_gtp(19), "D4");
        assert!(g.moves[2].point.is_none());
        assert!(g.moves[3].point.is_none());
    }

    #[test]
    fn follows_longest_variation_and_handles_escapes() {
        let sgf = "(;SZ[9];B[ee]C[a \\] bracket](;W[cc];B[gg])(;W[gc]))";
        let g = parse_game(sgf).unwrap();
        assert_eq!(g.moves.len(), 3);
        assert_eq!(g.moves[0].comment.as_deref(), Some("a ] bracket"));
        assert_eq!(g.moves[1].point.unwrap().to_gtp(9), "C7");
        assert!(g.warnings.iter().any(|w| w.contains("2 variations")));
    }

    #[test]
    fn prefers_longer_then_passes_then_later_branch() {
        // first branch short, second and third equal length; third ends with passes.
        let sgf = "(;SZ[9];B[ee](;W[cc])(;W[gc];B[cg];W[gg])(;W[gc];B[cg];W[]))";
        let g = parse_game(sgf).unwrap();
        assert_eq!(g.moves.len(), 4);
        assert!(g.moves[3].point.is_none(), "should follow the line ending in a pass");
        // equal length, neither passes: the later sibling wins
        let sgf = "(;SZ[9];B[ee](;W[cc];B[aa])(;W[gc];B[bb]))";
        let g = parse_game(sgf).unwrap();
        assert_eq!(g.moves[1].point.unwrap().to_gtp(9), "G7");
    }

    #[test]
    fn handicap_setup() {
        let sgf = "(;SZ[19]HA[2]AB[pd][dp];W[pp];B[dd])";
        let g = parse_game(sgf).unwrap();
        assert_eq!(g.setup_black.len(), 2);
        assert_eq!(g.who_moves_first(), Color::White);
    }

    #[test]
    fn gtp_roundtrip() {
        for s in ["A1", "T19", "J10", "D4", "Q16"] {
            let c = Coord::from_gtp(s, 19).unwrap();
            assert_eq!(c.to_gtp(19), s);
        }
        assert!(Coord::from_gtp("I5", 19).is_none());
    }
}
