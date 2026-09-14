# Go Teacher v5 — design brief for the analysis, report and lesson redesign

This brief is the shared context for the design and implementation agents. Everything here is
verified against the running system unless marked "to verify".

## What exists (v4)

Rust app (`/Users/aalekhsharan/code/go_teacher`): drives `katago analysis` over JSON lines
(`src/katago.rs`), analyses an SGF game (`src/analysis.rs`: pass 1 over all positions, optional
deep pass 2 over key positions, target-profile human policy pass at 1 visit), derives teaching
facts (`src/teaching.rs`: teaching candidates with theme, refutation = top reply of the next
position, better line, same-area move chains, status changes from ownership, per-phase facts;
`src/opening.rs`: corner shape names + first deviation; `src/progress.rs`: cross-game history),
renders a Markdown report (`src/report.rs`, `Report format: 4`) and a JSON dump, serves a local
web UI (`static/index.html`, live board with candidate/territory/policy layers, chart, variation
exploration via `/api/jobs/:id/explore`, engine settings, files, benchmark), and packages a
macOS app (tao + wry). Tests: `cargo test` including `tests/fake_katago.py`, a stand-in engine.

Lesson skill (`skill_indus/go-game-teacher`): `scripts/parse_review.py` (report → JSON, brief by
default), `scripts/validate_lesson.py`, `scripts/generate_lesson.py` (lesson JSON → single
self-contained HTML with canvas boards), `SKILL.md`, `references/*.md` (format, teaching
concepts, game-arc commentary with Go Magic library, html-build-guide). Runs in a sandbox with
python3, web search; the model executing it is a mid-size LLM (GLM 5.3 class). The **Markdown
report is the only input**; the SGF is not available to the agent.

Current lesson HTML per lesson: board + buttons Position / Show played move / Show better move /
Show all candidates / What it allows / Better line, plus alternative-move buttons, a story block,
explanation panel, level framing, principle. Puzzles: click-to-answer with graded wrong moves,
punishment sequences, hint, evaluations overlay. The user finds the many toggles confusing.

## User requirements for v5 (verbatim intent)

Add all of the following computations (do NOT remove anything existing):
1. Urgency via pass value: analyse the position with a pass appended; urgency = best − pass.
2. Endgame move values: value of each candidate = eval(play) − eval(pass).
3. Sente/gote classification of each move: is the opponent's best reply local or elsewhere.
4. Per-candidate ownership (`includeMovesOwnership`): region-wise plan differences played vs best.
5. Difficulty / sharpness: candidates within 1 point of best; policy flatness.
6. Local life-and-death status via `allowMoves`-restricted search around a group: can it live if
   attacked first, with which move; killing/saving move.
7. Small variation tree per teaching moment: top 2–3 candidates, 2–3 plies, each node evaluated.
8. Realistic refutations: opponent's reply chosen with the human network at the OPPONENT's rank,
   then evaluated — alongside the engine-best refutation.
9. Counterfactual replay: from a key mistake, play forward 20–30 moves with the human network at
   the student's rank for the student and the opponent's profile for the opponent; evaluate.
10. Missed opportunities: moments where a large gain was available and not taken.
11. Rank estimation from human-network likelihood of the student's moves, per phase.

Explicitly EXCLUDED: second opinion from another network; style vector; opponent-adjusted
severity; cross-game pattern mining. Do not change KataGo thread settings (already benchmarked).

Then: rethink the report so the agent gets a structured evidence pack per lesson and less bulk
("hand the agent less, not more"). Then recreate the skill (parser, validator, generator, docs)
to use every new element for lessons AND puzzles.

Lesson UI: replace the pile of toggles with a panel-based design. The user wants a
**human-style toggle**: view sequences/what-ifs from a human point of view at two levels — the
student's level and the target level (both already computed) — including realistic refutations.
"If I just click a panel, I should be able to do that." Clicking a panel should play the relevant
sequence on the board (numbered stones) and show its explanation.

## KataGo analysis engine facts (verified from docs + local runs)

- Query: `moves` [[color, "D4"|"pass"], ...], `initialStones`, `initialPlayer`, `rules`, `komi`,
  `boardXSize/YSize`, `analyzeTurns` (0 = initial position, k = after moves[k-1]; a position after
  a pass is a valid turn), `maxVisits`, `includePolicy`, `includeOwnership`,
  `includeMovesOwnership` (ownership array per candidate move), `includePVVisits`,
  `overrideSettings` {any config param, e.g. `humanSLProfile`}, `priority` (int; higher first),
  `avoidMoves` / `allowMoves`: list of {player: "B"|"W", moves: ["C3","pass",...],
  untilDepth: N}; allowMoves prohibits all moves EXCEPT those listed, at most one dict per player.
- Response per turn: `rootInfo` {winrate, scoreLead, scoreSelfplay, utility, visits, thisHash,
  symHash, currentPlayer}, `moveInfos` [{move, visits, edgeVisits, weight, winrate, scoreLead,
  scoreMean, prior, lcb, utility, order, pv, pvVisits?, ownership?, isSymmetryOf?}], `ownership`
  (row-major from top-left, `policy` same + pass last, -1 = illegal), `humanPolicy` when a human
  model is loaded and `humanSLProfile` is set (per query via overrideSettings).
- We launch with `-override-config reportAnalysisWinratesAs=BLACK`; winrate/scoreLead and
  (verified empirically) ownership are from Black's perspective.
- humanSLProfile names: `rank_20k`..`rank_1k`, `rank_1d`..`rank_9d`, `preaz_*` (pre-AlphaZero
  style), `proyear_1800`..`proyear_2023`, and asymmetric `rank_{BR}_{WR}` (Black rank BR vs White
  rank WR, each knowing the other's rank).
- Stronger human-style move choice (from docs): pick among moveInfos with probability
  ∝ humanPrior × exp(utility / 0.5).
- Cost model on this Mac: ~5 s per 500-visit position with the GPU shared; 1-visit human-policy
  passes are ~0.1 s per position; engine startup 60–90 s. Jobs run one at a time.

## Constraints

- Report must stay readable by a mid-size LLM: prefer structured, compact evidence per lesson
  over long move-by-move prose. Everything the agent may state about the board must be present
  in the report (no SGF access).
- Backwards compatibility: `Report format: N` line; the skill parser must check it.
- No new heavy dependencies in the Rust app; the lesson HTML must stay a single offline file
  with inline CSS/JS (canvas board renderer already exists in generate_lesson.py).
- Keep the live-view features; the lesson UI redesign is about the generated lesson page.

## Verified by live probes (2026-09-14, KataGo 1.18.0 Metal, 9x9 test position)

- `includeMovesOwnership: true` → every `moveInfos` entry carries `ownership` (boardYSize×boardXSize
  floats, Black perspective) and also `scoreStdev`, `utilityLcb`, `edgeWeight`, `playSelectionValue`.
- Appending `["B","pass"]` and analysing the next turn works: `currentPlayer` flips to W and the
  score lead dropped from B+1.6 to W+11.4, i.e. the pass value probe is meaningful.
- `allowMoves` with the same point list for both players restricts candidates to exactly those
  points (plus `pass` when listed). Evaluations under restriction differ from the unrestricted
  search (local reading, not global), so treat them as local verdicts only.
- `overrideSettings.humanSLProfile = "rank_10k_3k"` (asymmetric) returns `humanPolicy`.
- With a human profile set, `humanPolicy` is returned for every analysed turn regardless of which
  colour is to move, so per-colour likelihoods are available from one pass.
- `priority` is accepted (higher first) — useful to let short probe queries jump ahead of a
  long full-game query when both are in flight.
