# go_teacher Review Markdown Format

For **report format 5 / lesson schema 2**, read [evidence-format5.md](evidence-format5.md) first. Its evidence, perspective and authoring contract supersedes the legacy fields below. Keep the game arc, stories, resources, praise and practice structure described here.

Structure of the markdown produced by go_teacher (KataGo analysis). `scripts/parse_review.py` extracts everything below into JSON; this reference explains what the fields mean. The markdown is self-contained: nothing from the original SGF is needed.

**Format version.** Line 3 reads `Report format: N`. This document describes format 4; the parser also accepts 3 and 2. Reports without the line are treated as format 2.

Format history: 2 = teaching candidates, policy lines, compact entries; 3 = theme column, refutation and better line, chains, status changes, Game arc facts, two-pass markers; 4 = Opening patterns, Compared with earlier games, Time and loss / time spent, target human profile.

## Top-level structure

```
# Go game review: <title>
Report format: 3
## Game information            — metadata table, including the Student row and (two-pass) the number of deeply re-analysed positions
## How to read this document   — conventions (skip during parsing)
## Summary                     — accuracy, phases, game flow, biggest mistakes per player, turning points
## Teaching candidates         — the program's shortlist for the student, with board facts, refutation, chain + stone lists,
                                 then "### Good moves worth praising"
## Game arc facts              — per-phase numbers, then "### Life-and-death changes"
## Move-by-move analysis       — full entries for key moves (◆ = deeply re-analysed), one-line entries for the rest
## Opening patterns            — (13x13+) per corner: point names, approaches, invasions, first deviation
## Compared with the student's earlier games — (2+ earlier games) this game vs recent average, recurring themes, past games
## Final position              — KataGo's estimate and suggestions at the end of the record
## Appendix: compact move list — one line per move (canonical move list)
```

## Game information

Table with Black, White, Result recorded in SGF, Date, Board size, Handicap, Komi, Rules, Number of moves, **Student** (`Black (reason)` / `White (reason)`), Analysis strength, wall time. Player names may be missing (e.g. iPad exports); the Student row still says which side is being taught.

Winrates everywhere are **Black's**. Scores are `B+x` / `W+x`. Point loss is from the mover's perspective. Categories: best/excellent < 0.5, good < 1.5, inaccuracy < 3, mistake < 6, big mistake < 12, blunder ≥ 12 points.

## Summary

- **Accuracy by player**: moves, mean/median/total loss, match with top choice, top-3 match, category counts.
- **Point loss by phase**: opening / middlegame / endgame with move ranges.
- **Game flow**: Black winrate and score every 10 moves with a bar.
- **Biggest mistakes**: one table per player (introduced by `**Black (...)**` / `**White (...)**`): Move, Played, Loss, Winrate change, Category, KataGo preferred, Phase. Parsed into `summary.biggest_mistakes` with a `player` field.
- **Turning points**: winrate swings ≥ 15% or lead changes. In beginner 9x9 games nearly every move qualifies; do not treat this list as the shortlist.

## Teaching candidates

Begins with `Student: **Black (...)** (reason).` Then a table, one row per candidate (up to 8, the student's biggest losses with repeats from the same local fight removed):

| Column | Meaning |
|---|---|
| Move / Played / Better | move number, played move, KataGo's first choice |
| Loss | points lost |
| Theme | rule-based classification: reading / tactics, life and death, tenuki while threatened, over-defending / priority, shape / connection, endgame counting, direction of play, unclassified |
| Decided? | `yes (98.8%)` when Black's winrate before the move was outside 5–95%; then ignore winrate swings |
| Where the points went | board region (upper left, top side, ..., centre) and approximate points, from ownership maps |
| Better move is | `same area (dist d)` = shape/reading; `elsewhere (dist d)` = direction/priority (Chebyshev distance) |
| Stone captured? | `yes, move N` if the played stone was captured within 10 moves |
| Net policy | probability and whole-board rank of the played move in KataGo's raw network |
| Human policy | same for the human-style network at the chosen rank profile (`—` if no profile) |

Then one block per candidate, `### Candidate: move N (Colour MV)`, with bullet **hints** (plain-language facts: decided game, capture, tenuki while threatened, unnecessary local answer, region, groups in atari after the move, status changes caused by the move, "what the move allows", policy and human-policy sentences including the most common human move), followed by these structured lines:

```
- Position before the move (Black to play; opponent's last move F4): Black stones F7 F6 E5 F5; White stones D6 E6 D5 E4.
- Better line (Black first): D4 G4 C4 E2 B6 E8 E7 D7
- What the move allows (White first): F2 D8 G2 C4 D4 B3 C2 C7 → W+4.8
- Chain in this area — before: 8 White E4 (loss 5.1), 9 Black D3 (loss 9.1), ...; after: 16 White F2 (loss -0.1), 18 White G6 (loss 14.2; White E4 (4 stones) alive → dead), ...
- Theme: life and death. Phase: opening.
```

- **Better line**: KataGo's continuation after its preferred move (mover first). Parsed as `better_line`.
- **What the move allows**: the opponent's strongest reply and continuation after the played move, with the score it leads to — the punishment. Taken from the analysis of the next position (deep in two-pass mode). Parsed as `refutation` / `refutation_score`.
- **Chain**: moves within three points of the played move in the eight moves before and twelve after (at most six each side), with point loss and notes: `captured at move N`, `<Colour> <anchor> (<n> stones) <from> → <to>` status changes. Parsed as `chain_before` / `chain_after`, each entry `{move_number, color, move, point_loss, note}`.
- **Status hints**: "This move changed the status of the Black group at D3 (2 stones): alive → dead." Parsed as `status_changes` on the candidate.

Use the position line to reason about the position without a diagram, and as the base for describing the lesson. Puzzles must not reuse it.

`### Good moves worth praising`: table of Move, Played (`Black G8`), Next best, Gap (pts), Black winrate before — moves where the student found the only good move in a live position.

## Game arc facts

A table with one row per phase the game had:

```
| Phase | Moves | Black winrate | Score | Student mean loss | Opponent mean loss | Student worst | Opponent worst | Student top-1 | Hot regions |
| opening | 1–16 | 27.9% → 0.3% | W+0.4 → W+4.7 | 4.95 | 4.45 | 15 G4 (9.6) | 10 C3 (9.1) | 25% | centre (9), bottom side (8) |
```

Winrate and score are at the start and end of the phase; "worst" is the biggest single loss per side (move, coordinate, points); "top-1" is how often the student found KataGo's first choice; "hot regions" are the board regions (upper left, top side, upper right, left side, centre, right side, lower left, bottom side, lower right) where ownership changed most over the phase, with the summed change in points. Parsed as `arc.phases`.

`### Life-and-death changes`: a table `| Move | Played by | Group | Stones | From | To |` of groups (two stones or more, or a captured group) whose predicted owner changed with a move — statuses `alive`, `dead`, `unsettled`, `captured`. Parsed as `arc.status_changes`. This is the only source you may use for "this group died / was saved / was captured".

## Time and loss (Summary, when the record has clocks)

`### Time and loss`: the median thinking time and, per player, the number of fast (≤ median) and slow moves with their mean loss. Full move entries then carry `- Time spent on this move: N s.`, and candidate blocks a hint "Played in N s, among the fastest quarter ..." or "... slowest quarter ...". Parsed as `time_and_loss`, `move_details[n].time_spent_seconds`, `candidates[].time_spent_seconds` / `fast`.

## Opening patterns (boards of 13 lines or more)

One `### <corner> corner: <summary>` block per corner that saw play in the first 50 moves. Each line: `- Move N Colour MV: <description> (loss X, KataGo's choice | KataGo's #k | not searched)`. Descriptions are derived from the stones: `4-4 point`, `3-4 point`, `small knight's approach (3-6 point)`, `one-space jump / high approach`, `3-3 invasion`, `attachment to the previous stone`, ... Then `First deviation: move N MV (KataGo preferred X, L points).` or `No move in this corner lost 1.5 points or more.` There is **no joseki database** behind this: the names describe shapes, and KataGo judges each move; research the named pattern online for the standard sequences. Parsed as `openings[]` with `corner`, `summary`, `moves[]`, `first_deviation`.

## Compared with the student's earlier games (once two or more earlier games exist)

A metric table (this game vs the recent average of up to 10 earlier games: mean loss, loss per phase, top-1 rate, moves losing 3+), a `Recurring themes in recent games` line (teaching-candidate themes counted across those games), and a table of the past games (name, date, opponent, result, mean loss, top-1). Parsed as `history`. Same-student matching uses the player name when the SGF has one, else the colour.

## Target human profile

When a second, stronger profile was chosen at upload, policy lines gain `Target profile: p% (#k), most common move X (q%)` and candidate hints gain `Players at the stronger target profile play this move ...`. Parsed as `move_details[n].target_policy_*` and `candidates[].target_policy`.

## Move-by-move analysis

**Key moves** (teaching candidates, praised moves, the opponent's six biggest mistakes, checkpoints, the last move) have full entries; in two-pass mode a `◆` after the heading marks a move whose before and after positions were re-analysed deeply (parsed as `deep: true`):

```
### Move 15: Black G4
- Evaluation: Black winrate 95.3% → 0.2%, score B+6.2 → W+4.4.
- Point loss for Black: +10.6 pts (KataGo's #3 choice); winrate change for Black: -95.0%. Verdict: **big mistake**. Phase: opening.
- KataGo's expected continuation after G4: ...
- KataGo preferred **D4** (95.2%, B+6.1), expecting: D4 G4 C4 E2 D2 E8 F8 B5
- Network policy for G4: <0.1% (#2 over the whole board). Human policy: 17.5% (#3), most common human move D4 (40%).
- Comment in the game record: ...            (only human comments; automatic KaTrain comments are dropped)

Candidates in the position before this move:
| # | Move | Black winrate | Score | Loss vs best | Visits | Policy | Expected continuation |
```

`Loss vs best` is how many points worse the candidate is than KataGo's first choice, from the mover's view (`best` for the first choice). Use it to grade alternatives. `Visits` is search effort: a candidate with 1 visit is a glance, not an evaluation. PVs are capped at 6 moves in tables.

Diagrams (`Position after move N:`) appear for key moves that lost 3+ points and at checkpoints: `X` Black, `O` White, `+` star point, `#`/`@` the Black/White stone just played, `1 2 3` KataGo's alternatives.

**Other moves** are single bullet lines:

```
- **Move 7** Black F5: loss 3.3 (unsearched), then Black 17.3% / W+0.9.
```

(loss, KataGo's rank of the move or `unsearched`, then Black's winrate and the score after the move.)

## Final position

KataGo's estimate after the last move and its suggested next moves (same table format).

## Appendix: compact move list

```
  1 B E5    loss  -0.1  rank #1   wr  32.8%  W+0.3
```

Move number, colour, coordinate, point loss, rank (`#N` or `-` = unsearched), Black winrate after, score after. The parser uses this as the canonical move list.

## Coordinates

GTP: columns A–T skipping I, rows 1–N from the bottom. On 9x9, columns A–J, `E5` is the centre. `pass` is a pass.

## Note on PVs

A PV is what KataGo expects both sides to play. The human-explainable reason a move fails often lies in a line where one side "doubles down", which is not the PV. When you demonstrate such a line, label it as your own illustration.
