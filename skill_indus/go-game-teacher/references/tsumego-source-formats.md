# Tsumego source formats: exact, citable problem positions

Use this when research (workflow step 4) or puzzle design (step 6) needs a classic
problem as an external example with exact coordinates. It records what the
machine-readable tsumego collections look like, how to parse them, and the
specific decoding traps we have hit, so you never have to reverse-engineer the
formats again. It complements grounding-and-practice.md: that file defines what
a sourced puzzle must contain; this one is where the source positions come from.

## Source map

| Source | What it gives | Solutions | Coordinates | Status |
|---|---|---|---|---|
| `tasuki/tsumego` (github.com/tasuki/tsumego) | 4,341 classic problems as TeX: Cho Chikun's Encyclopedia 900/861/792, Gokyo Shumyo 520, Xuanxuan Qijing 347, Igo Hatsuyo-ron 183, Lee Chang-ho 738 | No (copyrighted) | tex `books/problems/*.tex` | **Primary source.** Parse with `scripts/parse_tasuki_tex.py` |
| `Seon82/tasuki2sgf` (github.com/Seon82/tasuki2sgf) | The same 4,341 problems pregenerated as SGF (`generated/*.sgf`, one multi-problem file per book) | No | SGF `AB[]/AW[]/PL[]` | Easiest to consume; 1 known defect (below) |
| `gogameguru/go-problems` (github.com/gogameguru/go-problems) | ~420 "weekly go problems" (easy/intermediate/hard) with full solution trees, by An Younggil 8p | **Yes** | SGF under `weekly-go-problems/` | Use when you need a verified solution, not just a position. CC BY-NC-SA 4.0 — credit and link |
| 101books (101books.github.io) | 107 booklets, 23,536 problems scraped from 101weiqi | In the built PDFs | `.gnos` TeX-ish line files (`problems/<book>/…`) | Huge, no Cho Chikun; scraped without stated rights — reference only, do not redistribute |
| cho2sgf (Bill Shubert) | Cho Chikun encyclopedia from the Kiseido diskette | Yes | SGF | Requires the commercial diskette; not fetchable |
| mango314/scrapeGo | Cho/classics scraped from the rendered PDFs | No | SGF | **Do not use**: positions are transposed (diagonal reflection) and several problems are wrong; tasuki rejected it |
| Sensei's Library (senseis.xmp.net) | Names, explanations, proverbs for shapes | Prose | **Diagrams are images** — you cannot transcribe coordinates from them | Use for concepts and citations, never for exact stones |
| Wikipedia | Atari, snapback, ladder, etc. definitions | Prose | n/a | Search precisely: "Atari (go)", "Snapback (go)" — bare "atari" hits the game company |

The tasuki corpus is the backbone: original SGFs were lost, so the `.tex` files
are the surviving ground truth (tasuki generated them with sgf2dg in 2004;
problems were rotated to the upper-left corner and published as PDFs at
tsumego.tasuki.org). Fetch the tex files directly:

```
https://raw.githubusercontent.com/tasuki/tsumego/master/books/problems/cho-1.tex
```

(books: `cho-1`, `cho-2`, `cho-3`, `gokyoshumyo`, `hatsuyoron`, `xxqj`,
`lee-chang-ho`.)

## The tex format

Each problem is a block:

```
\vbox{\vbox{\goo
<one line per board row, cells concatenated>
}
\hfil problem 1\hfil\break
}
```

The title after `\hfil` is the problem's identity: `problem 1`, or for
Lee Chang-ho `volume 6, problem 4`. A title containing "white to play" means
White moves first — this occurs in the classical books (Gokyo Shumyo 264/520,
Xuanxuan Qijing 206/347, Hatsuyo-ron 88/183) and never in Cho or Lee Chang-ho,
which are all black to play despite the prefaces claiming it for every book.

Each cell is one of five TeX macros (G = the grid glyph, L = a label):

| Macro | Meaning |
|---|---|
| `\- @G` | black stone on the point with glyph G (note: backslash, dash, **space**) |
| `\- !G` | white stone on that point |
| `\0??G` | empty point |
| `\0LLG` | empty point with 2-char label LL (e.g. `\001(`; label `??` means none) |
| `\!  L` | empty point whose label letter L replaces the glyph |

Grid glyphs carry no stone information: `+` interior, `[` `]` left/right edge,
`(` `)` top/bottom edge, `<` `>` corners, `*` hoshi (star point), `.` `,` rare
corner variants. Real example, Cho elementary problem 1, first two lines:

```
\0??<\0??(\- !(\0??(\0??( ... \0??>          <- top edge, one white stone
\- ![\- !+\- !+\- !+\- @+\- @+\0??+ ... ]    <- left edge, 4 white + 2 black
```

### Decoding rules

Walk a row left to right. Each cell is exactly one point and advances the
column by one. A stone marker (`@` black, `!` white) rides **on the following
glyph's point** and never advances the column by itself. A label char (from
`\!  L`) is itself the point. Concretely:

1. Strip nothing globally. Tokenise: `\- @`, `\- !`, `\0??`, `\0<LL>`, `\!  ` are
   cell prefixes; each prefix plus its payload is one point.
2. Record `@`→black, `!`→white on the current column; `\0<LL>` with LL ≠ `??`
   and `\!  L` record labels.
3. **Every non-empty row must decode to exactly 19 points.** If it does not,
   you have mishandled a macro — stop and fix, do not eyeball past it. (Also
   `row.strip()` first: one hatsuyoron row ends with a stray U+3000
   ideographic space.)
4. Blocks are found with `^\\vbox\{\\vbox\{\\goo\n([\s\S]*?)\}` (MULTILINE) —
   note the lazy match to the first `}`; rows never contain `}`. Lines starting
   with `%` are TeX comments and must not be treated as problems (hatsuyoron
   contains commented-out broken blocks).

Use the shipped parser rather than rewriting this:

```
python <sandbox_dir>/scripts/parse_tasuki_tex.py cho-1.tex -o cho-1.json
```

It reads a local file or URL, emits JSON with GTP **and** SGF coordinates,
labels and player-to-move, asserts the 19-point row width, and was verified
against the pregenerated SGFs: 4,340/4,341 problems identical on stones and
player-to-move.

### Coordinates

The first tex line of a problem is its **top** row (the `<` `>` corner glyphs
and sgf2dg's top-down typesetting confirm it). Problems are fragments of a
19×19 board: always the full 19 columns wide, usually only 4–6 rows tall,
hugging the top edge and left edge.

- tex line N (0-based) → SGF row letter `chr(N+97)`; tex column C (0-based) →
  SGF column `chr(C+97)`. SGF is top-left origin: first letter = column,
  second = row, `aa` = top-left.
- GTP (the lesson/puzzle format): column = `ABCDEFGHJKLMNOPQRST[C]` (no I),
  row number = `19 - N`. So SGF `da` (Cho 1, problem 1) = GTP **D19**.
- Cho problems therefore have stones in SGF rows `a`–`h` (GTP rows 19 down to
  12). If your parse scatters them across `q`–`s` instead, you flipped the
  board — see pitfall 2.

## Pitfalls we have already hit

Each of these cost a debugging cycle before; the parser above avoids them all.

1. **Treating the tex as a bitmap matrix.** It is macro text, one cell per
   point. Decode cell-by-cell with the grammar above, never by grepping for
   grid characters.
2. **Vertical flip (the "rows a-h, not q-s" bug).** Expecting GTP-style
   bottom-origin rows makes you read the first tex line as row 19 and flip
   every stone to the mirror row. The SGF row letter counts from the *first*
   tex line downward. Symptom: stones land in `q`–`s` instead of `a`–`h`.
3. **Regex stone extraction dropping stones.** A `findall` over `\- @`/`\- !`
   pairs silently drops the first stone of a colour in a row (the left-edge
   cell `\- ![` looks different from interior `\- !+`) or mis-pairs a stone
   with the wrong glyph. Use the token walk; if you must regex, verify the
   decoded stone count against a rendered board before using the data.
4. **Advancing the column on stone chars.** Stones ride on the following
   glyph; only glyphs and label chars advance. Advancing on both shifts every
   stone after the first in a row. Symptom: whole rows skew right.
5. **Unhandled `\0<LL>` label macros.** Lee Chang-ho "volume 6, problem 5"
   contains `\001(` (a point labelled "01"). The pregenerated SGFs and any
   naive strip treat its leftover characters as extra columns, shifting the
   rest of that row +4 columns (stone `la` becomes `pa`). Our parser handles
   it correctly and records the label. If you consume the shipped SGFs, treat
   that one problem as unreliable.
6. **scrapeGo is transposed.** Its SGFs are a diagonal reflection of the tex
   ground truth (885/900 problems, rest corrupted). Both describe playable
   positions (a reflection is a legal symmetry), but only the tex/tasuki2sgf
   orientation matches the printed books — cite from ours.
7. **There are no solutions anywhere in the tasuki corpus.** The books
   deliberately omit them (copyright). Never cite a "solution" to a Cho
   problem from a source you have not verified; solve it yourself and prove
   it with the bounded capture checker or `verify_puzzle_engine.py`, as
   grounding-and-practice.md requires. A few problems in the printed books
   are known to be unsolvable — if your solver finds nothing, suspect the
   problem, then your parser, then the objective.
8. **Don't cite from memory.** "Cho elementary problem N" claims must come
   from the parsed data (the `source.book` + `problem` fields), not from
   recollection of the book.
9. **Abandoning verification when a search feels slow.** Symptom: "the
   exhaustive search is too slow, I'll inspect the problems visually and
   design small, quickly-verifiable puzzles." That substitutes eyeballing
   for the crop-and-verify pipeline. The documented response is in
   [When the search is slow](#when-the-search-is-slow--do-not-improvise):
   `solve_tsumego.py --scan` (near scope), then confirm the seed in full
   scope, then lower plies or pick an easier problem. Never author a puzzle
   whose objective was not verified mechanically.

## Workflow for puzzle cores

Two scripts do the whole pipeline. They live in `scripts/` next to
`go_rules.py` and `parse_tasuki_tex.py`, and take a local file or a raw
GitHub URL:

```bash
# 1. fetch and parse a book (once per session)
python <sandbox_dir>/scripts/parse_tasuki_tex.py \
  https://raw.githubusercontent.com/tasuki/tsumego/master/books/problems/cho-1.tex \
  -o cho-1.json

# 2. scan a problem range for seeds whose target has exactly one verified
#    key move (near scope: fast screening, ~1-2 min for 60 problems)
python <sandbox_dir>/scripts/solve_tsumego.py --json cho-1.json --scan 1:60 --plies 5

# 3. verify the chosen seed and emit a puzzle-ready JSON (full scope by
#    default: sound, all legal replies, ~20s)
python <sandbox_dir>/scripts/solve_tsumego.py --json cho-1.json --problem 11 \
  --mode capture --target F19 --plies 5 -o seed.json
```

`--mode survive --swap-colors` finds defender puzzles (the student saves a
group the original problem kills). The emitted seed JSON carries the board
size, stones, player, a ready `objective` and `verified_moves` in
cropped-board GTP coordinates, plus the citation fields (`seed.book`,
`seed.problem`, `seed.title`). Difficulty by book: Cho elementary ~13k,
intermediate ~5k, advanced ~1k, Gokyo Shumyo ~1d, Xuanxuan Qijing ~2d, Igo
Hatsuyo-ron ~7d.

Then transform (rotate/mirror/swap colours/move corner) per step 6 of
SKILL.md and grounding-and-practice.md, and re-verify on the *transformed*
board — the source position's solution proves nothing about your variant.
Cite: `source.url` = the repo or book page, `source.example_locator` = book
+ problem title (e.g. "Cho Chikun's Encyclopedia of Life & Death, Elementary,
problem 11"), `source.position` from the parsed JSON.

### When the search is slow — do not improvise

Exhaustive search on open boards is slow. That is an expected property of
the problem, already solved by the crop in `solve_tsumego.py` — it is never
a reason to change method. If anything feels slow, in order: run `--scan`
(near scope, seconds per problem), verify the chosen seed on the default
9×9 crop in full scope, lower `--plies`, or pick an easier problem. Two
hard rules:

- **Never fall back to inspecting diagrams and designing "small, quickly
  verifiable" puzzles by eye.** Visual inspection may *select* a seed; stone
  coordinates come only from the parser output, and every objective claim
  comes only from `solve_tsumego.py` / `validate_lesson.py`. An unverified
  puzzle is not a shortcut — it is a broken deliverable.
- **Treat the scan's near scope as screening only.** In cross-validation it
  disagreed with the full scope on about 1% of moves (a defender escape ran
  outside the restricted move set, turning a failed move into a false win).
  Single-problem runs default to the sound all-legal-replies scope; trust
  uniqueness and failure claims only from a full-scope run, and confirm any
  seed that a scan proposed.

For shapes and teaching prose around the position, Sensei's Library and Go
Magic remain the right references (step 4) — just do not try to transcribe
coordinates from Sensei's image diagrams.

## SGF problems

For sources that ship solution trees as SGF (gogameguru/go-problems, tasuki2sgf
collections, exports from OGS/GoGui/Sabaki), use `scripts/parse_sgf_problem.py`
instead of hand-reading the file:

```bash
python <sandbox_dir>/scripts/parse_sgf_problem.py problem.sgf -o puzzle.json
python <sandbox_dir>/scripts/parse_sgf_problem.py collection.sgf --index 3 -o puzzle.json   # 0-based game index
python <sandbox_dir>/scripts/parse_sgf_problem.py collection.sgf --all -o all.json          # list, same shape as parse_tasuki_tex.py
```

It emits the same dict as `parse_tasuki_tex.py` (`title`, `player_to_move`,
`black`/`white` GTP lists, `sgf_black`/`sgf_white`, `labels`, `source`) plus
the solution fields the puzzle schema wants, so `solve_tsumego.py --json` can
consume the `--all` output directly.

| SGF | Output field | Notes |
|---|---|---|
| `SZ[n]` / `SZ[w:h]` | `board_size` (+ `board_size_y` if not square) | default 19, or `--size` |
| `AB[..]` / `AW[..]` (root, or a move-less first node; `aa:cc` compression and `AE` handled) | `black`, `white`, `sgf_black`, `sgf_white`, `source.position.{black_stones,white_stones}` | GTP columns skip **I**; SGF row `a` is the TOP, so `da` on 19x19 = `D19`, on 9x9 `ai` = `A1` |
| `PL[B/W]`, else colour of the first move, else B | `player_to_move` | a warning is recorded when defaulted |
| root `C[..]` | `description` | |
| `GN` / `EV` | `title` | else `problem <index+1>` |
| `LB[pt:text]` | `labels` | |
| variations | `correct_moves`, `correct_lines`, `wrong_moves[{move, refutation, explanation}]` | see classification below |
| file path + game index | `source.file`, `source.problem`, `source.example_locator` = `"<path> problem <n>"` | rewrite the locator to the publisher's own numbering before citing |
| `tt` or `[]` move | `"pass"` in a line | `go_rules.Position.play` accepts `pass` |

Classification: every leaf path is read from the *last node's comment*. A
WRONG marker (`wrong`, `incorrect`, `fail…`, `bad`, `✗`, `not correct`) wins over
a CORRECT marker (`correct`, `right`, `✓`, `RIGHT`, `success`, `solution`), so
"incorrect" is never read as correct. A first move is correct when at least one
leaf below it is correct; `correct_lines[move]` is the longest correct path; any
other first move becomes a `wrong_move` whose `refutation` is the longest
continuation and whose `explanation` is that leaf's comment. If **no** leaf in
the tree carries a marker, the first child at the first real branch point is
assumed correct and a warning is recorded — check it by hand, some collections
put the solution last.

Caveats:

- The parser records `warnings` for non-alternating colours (a variation that
  shows "if the opponent tenukis") — `go_rules` cannot replay those lines, so
  drop or split them before `check_puzzle`.
- Comments are free text: a comment like "correct shape but wrong timing" is
  classified wrong. Read the leaf comments in the output before trusting labels.
- The output is a **source position**, not a puzzle. `check_puzzle` will (and
  should) still reject it: it needs `transformation`, `objective`,
  `solution_review`, `board_checks`, and the transformed board must be
  re-verified with `solve_tsumego.py` / `verify_puzzle_engine.py`. The SGF
  author's variations are a hint for the objective, not a proof.
- gogameguru problems are CC BY-NC-SA 4.0: keep `source.url` pointing at the
  problem page and credit An Younggil / Go Game Guru.

## OGS puzzles

`scripts/parse_ogs_puzzle.py` converts a puzzle from online-go.com's public API
(`https://online-go.com/api/v1/puzzles/<id>`), either fetched by id or from a
saved JSON file (tests use files only; nothing else in the skill hits the
network without being asked):

```bash
python <sandbox_dir>/scripts/parse_ogs_puzzle.py 45 -o puzzle.json               # fetches the id
python <sandbox_dir>/scripts/parse_ogs_puzzle.py saved_puzzle_45.json -o puzzle.json
```

Verified API shape (puzzle 45): top-level `id`, `name`, `collection{id,name}`,
`owner{username}`, `private`, `rating`, `type`; and `puzzle{puzzle_rank
(string), puzzle_type, width, height, initial_state{black,white},
initial_player, puzzle_description, move_tree}`. `initial_state.black` is a
single string of letter pairs, two letters per stone, `a` = 0, **x then y, y
counted from the top** (`"daaa"` = D19 and A19 on 19x19). `move_tree` is a
root `{x:-1,y:-1}` with `branches:[{x, y, correct_answer?, wrong_answer?,
text?, branches:[…]}]`; `x = -1` inside the tree is a pass.

| OGS | Output field |
|---|---|
| `width`, `height` | `board_size` (+ `board_size_y`) |
| `initial_state.black/white` | `black`, `white` (GTP, no I), `sgf_*`, `source.position` |
| `initial_player` | `player_to_move` |
| `puzzle_description` | `description`; `name` → `title` |
| first-ply branch flagged `correct_answer`, or with a `correct_answer` node below it | `correct_moves`; `correct_lines[move]` = deepest such path |
| every other first-ply branch | `wrong_moves[{move, refutation = deepest continuation, explanation = branch/leaf text}]` |
| `puzzle_rank` | `rank_hint{puzzle_rank, note, tentative_label}` |
| `id`, `name`, `owner`, `collection`, `private`, `rating`, `type`, `puzzle_type` | `source.url` (API URL), `source.web_url`, `source.example_locator` = `"OGS puzzle <id> (<name>)"`, `source.owner`, `source.collection_id`, … , `source.credit` |

Caveats:

- **Rank semantics are unconfirmed.** `puzzle_rank` is a string; `"0"` appears
  to mean unranked. The `tentative_label` assumes OGS's internal rank numbering
  (r < 30 → (30−r) kyu, r ≥ 30 → (r−29) dan) — treat it as a hint only, never
  as a rating claim in a lesson. `rating` is the community star rating, not a
  difficulty.
- **Credit.** OGS puzzles are user-contributed; the CLI prints a reminder to
  stderr and the output carries `source.credit` (owner + OGS). Check the
  collection's terms before reusing a puzzle, and respect `private: true`.
- Unflagged leaves count as wrong (that is how the OGS client treats moves
  that do not reach a `correct_answer`), and a branch carrying both flags is
  reported in `warnings`. Puzzles whose tree has no `correct_answer` at all
  yield an empty `correct_moves` plus a warning.
- As with SGF problems, the result is a source position only: transform,
  verify the *transformed* board mechanically, fill `objective`,
  `solution_review`, `board_checks`, then run `check_puzzle`.
