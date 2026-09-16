---
name: go-game-teacher
description: "Transforms go_teacher/KataGo game review markdown into interactive HTML lessons for beginners. Use this skill whenever a user uploads a Go game review, analysis, or KataGo/KaTrain/go_teacher markdown and wants to learn from their mistakes. Triggers on phrases like 'analyse this Go game', 'review my Go game', 'help me improve at Go', or any mention of KataGo/KaTrain game reviews. Produces a self-contained HTML file with clickable board positions, explanations of the played move and the better plan, grounded praise for moves played well, practice puzzles from researched external examples at the student's level, and a phase-by-phase narrative of the game, grounded in web research with a focus on Go Magic tutorials."
allowed-tools: execute_code terminal read_file write_file workspace_output workspace_emit present_output web_search web_get_contents
---

# Go Game Teacher

Turn a go_teacher review markdown into an interactive HTML lesson that tells the student what
level they played like, what they got wrong and why, what they did well, and lets them practise
the same idea on fresh puzzles at their level. The game overview and phase arc give context; the
2–3 teaching moments get most of the attention. Never repeat one takeaway in the overview, a phase
and a lesson.

The main `.md` report is the **only game input**. Do not ask for the app's `.json` or the SGF.
`parsed.json`, `brief.json`, `facts.json`, slices and parts are working files this skill creates.
Every board fact comes from the report's evidence block; every number you write is copied from a
field, never computed or remembered.

The parser accepts report format 5 with `evidence_version` 1 or 2 (version 2 adds
`student_profile`, praise kinds, `*_with_colours`, `stones_captured` and, in newer reports, the
search-coverage fields `search_coverage`, `evaluation_coverage`, `investigation_coverage`; see
`references/evidence-format5.md`). If it refuses the file, tell the user the skill and go_teacher
are out of step; do not guess. Formats 2–4 are legacy (`references/katago-review-format.md`).

## What you need

- **Input**: one markdown file produced by go_teacher. **Output**: one self-contained HTML file.
- **Read first**: `references/evidence-format5.md` (data contract, schema 2),
  `references/grounding-and-practice.md` (facts, fact_checks, puzzle verification),
  `references/level-ladder.md` (rank bands, puzzle tiers, prose register).

## Workflow

Steps 1–4 are decisions; 5–11 are authoring; 12–13 are checks and output. Keep the order.

### 1. Parse the review

```
python <sandbox_dir>/scripts/parse_review.py <input.md> <parsed.json> --brief-output <brief.json> --facts-output <facts.json>
```

Read brief.json to choose moments, parsed.json for their complete evidence; always pass
parsed.json to validation and generation. After choosing moments, rebuild a small facts file:
`python scripts/teaching_facts.py parsed.json facts.json --moves 19 29` (or `parse_review.py …
--moves 19 29`). For a team run, slice with `scripts/slice_for_agent.py` (see "Run as a team").

### 2. Identify the student

Read `game_info.student` (`"B"`/`"W"`) and `game_info.student_reason`. Write "The student is
<Black/White> because <reason>" and use it in the overview. Every lesson, praised move and
statistic refers to that side. Never assume Black.

### 3. Read the student profile and state the level

Read `student_profile`. Copy `estimate.wording` ("plays like a 12k in this game (range 15k–10k,
33 moves)") and `caveat`. Working rank: `working_rank.rank_label`, else `estimate.rank_label`,
else `profiles_used.student` with its `student_source`; look up the band in
`references/level-ladder.md`. The overview states the level once, as "plays like …", with the
caveat; never "you are a 12k". Without `student_profile` (version 1), use the upload profile and say so.

### 4. Choose 2–3 teaching moments with the ladder

Start from `teaching.candidates` (already ranked by point loss, repeats removed). Apply the
selection rules in `references/level-ladder.md`: concept band at or one above the working rank;
prefer the weakest phase (highest student mean loss in `arc.phases`); then point loss; distinct
`theme` values; prefer a non-empty `refutation_with_colours`. A candidate two bands above the
rank becomes one "later" sentence in the overview, not a lesson. Use the candidate's `theme` as
the lesson theme; override only with a stated reason. Fall back to `summary.biggest_mistakes`
filtered by player only if fewer than two candidates exist.

Candidate fields you will quote: `point_loss`, `loss_region`, `captured_at`, hints,
`human_policy` / `target_policy`, `refutation_with_colours`, `better_line_with_colours`,
`status_changes`, `chain_before` / `chain_after`, `difficulty`, `evaluation_coverage`,
`investigation_coverage`, `unavailable` (fewer panels).

Coverage (`facts.coverage[N]`, one line per candidate): when `evaluation_coverage.status` is
known, prefer candidates with `complete` for the primary lessons. A `complete` candidate whose
`investigation_coverage.status` is `not_selected` (or whose `evidence.unavailable` has
`teaching_investigations`, legacy `deeper_search`) is fully teachable from `refutation_with_colours`,
`better_line_with_colours`, the policies and the chain — just without `local` / `what_if`
panels. Say "verified" or "deep evaluation" only for a `complete` candidate; an `incomplete` or
`unknown` one (any older report) gets neither word. Never write a human reply, restricted reading
or what-if line that is not in the candidate's `evidence.sequences`. The learner does not get a
coverage section; the validator checks these claims.

### 5. Trace the arc

Use `references/game-arc-commentary.md`. `arc.phases` gives ranges, scores, mean loss and hot
regions; `arc.status_changes` and `moves[].stones_captured` give captures. Around each chosen
move, trace backward through `chain_before` and forward through `chain_after`, `captured_at` and
the refutation; connect only chain and status entries. Phase labels are move-count heuristics;
never say the game was decided from a saturated winrate.

### 6. Research (uncapped, Go Magic first)

For each moment: the concept row in `references/level-ladder.md` names the Go Magic resource
(URLs only from the verified library in `references/game-arc-commentary.md`), the Sensei's
Library page and the puzzle tier. Then run the sequential searches in game-arc-commentary.md
(phase, chain, pattern name, classic example). Read results in full; distil 2–3 bullets per
moment. No search-count, source-count or time cap. Exact puzzle positions come only from the
machine-readable sources in `references/tsumego-source-formats.md` and level-ladder.md table 2
(`parse_tasuki_tex.py`, `parse_sgf_problem.py`, `parse_ogs_puzzle.py`), never from images.

### 7. Write each lesson (schema 2)

Write prose; the generator loads numbers, boards and sequences from parsed.json. Per lesson:

- `move_number`, `title`, `concept_label`, `theme` (copied), `story` (2–4 sentences from the
  chain and status facts, with move numbers), `principle` (one habit sentence).
- `panels.position`: what to notice before moving.
- `panels.played`: what the move allows. Walk `refutation_with_colours` (or the played
  sequence's `moves_with_colours`) by **copying the tokens in order**: "White plays W F3, Black
  answers B D7, then W H8". Never alternate colours yourself; never list bare coordinates.
- `panels.best`: walk `better_line_with_colours` the same way and say why the plan works.
- `panels.alternatives`: compare `move_details[N].candidates` by `loss_vs_best`; a 1-visit move
  is "barely searched", not a precise number.
- `panels.local`, `panels.what_if` (optional): restricted reading with the defender named and
  "prediction, not proof"; sampled futures with the later-choices caveat. Omit if ungrounded or
  when `investigation_coverage.status` is `not_selected` / `disabled` / `unavailable`.
- `level_framing`: from `facts.lessons[N].policy` — the student profile's probability for the
  played move and the target profile's for the preferred move, each bound to its move, direction
  checked ("the 12k profile gives F1 about 1 in 1000; the 8k profile prefers H4 at 78%").
- `fact_checks`: one per number, colour, defender, location or status you assert (format in
  grounding-and-practice.md). At least one per lesson.
- Optional `sequence_explanations` (by sequence id); `alternative_explanations` (by searched move).

Do **not** write `alternatives`, `played_quality`, `concept`, `refutation`, `better_line`,
`point_loss`, `played_move`, `preferred_move` or `player_color` in a lesson: schema 2 forbids or
ignores them. Use the band's register (level-ladder.md): below 10k, gloss every Japanese term
inline at first use, no engine jargon, one idea per sentence.

### 8. Praise: 3–4 good moves strictly from `teaching.praise`

Take entries from `teaching.praise` in order (up to 4). Each `good_moves` entry: `move_number`,
`move`, `kind_label`, `explanation`, and `rating` + `rating_source` **copied verbatim** when both
exist (omit both otherwise). The explanation opens by copying `note`, then states
`stones_captured` when > 0 and `gap` when ≥ 1, then — only when `human_prob` < 0.25 — why it was
not obvious ("players at your level choose it about N% of the time"), then one habit sentence.
If fewer than 3 entries exist, add student moves from `moves[]` with `stones_captured > 0`,
stating the capture and nothing more. Never praise any other move; never write "the engine
praised" for a move absent from `teaching.praise`; never compose a kyu/dan rating.

### 9. Design one puzzle per lesson

Rules (details in `references/grounding-and-practice.md` and level-ladder.md table 2):

- From a researched external example in the band's tier, parsed with a script; never the game
  position or its symmetry/colour swap; the transformation must change the reading.
- One explicit `objective` (`capture_within`, `avoid_capture_for`, `compare_plans`),
  `solution_review` with strongest defences, `board_checks` for every tactical claim.
- 2–3 `wrong_moves` (`quality`, `explanation`, legal `refutation` starting with the opponent's
  reply); `correct_lines` per correct move; `generic_wrong_explanation`; `source` (`url`,
  `title`, `example_locator`, `position`), `transformation`, `verification`,
  `transfer_explanation`, `lesson_move_number`, `evidence_focus`, `hint`, `explanation`,
  `opponent_last_move`, `player_to_move`.
- Verify with `scripts/solve_tsumego.py` or `scripts/verify_puzzle_engine.py`; inconclusive
  means redesign, not relabel.

### 10. Concepts and resources

One `concepts_learned` entry per lesson concept: `term`, `japanese` (from the table in
`references/go-teaching-concepts.md`), `description` (2–3 sentences in the band's register),
`resources` (Go Magic first, Sensei's second, at most 3), `anecdote` copied from the curated
list in go-teaching-concepts.md or omitted. No other anecdote source.

### 11. Overview, arc and progress

- `overall_feedback` (2–3 short paragraphs): the student's side, the level sentence with its
  caveat, main strength, main weakness, weakest phase — from `summary.accuracy` and `arc.phases`.
- `game_arc`: `intro` plus one entry per phase (`phase`, `move_range`, `narrative`, optional
  `turning_points`, `anchor_move`). Captures only from `moves[].stones_captured` or
  `arc.status_changes`, with the stone count.
- `progress.summary` from `facts.current` and `facts.history.prior_games`; the renderer fills
  the rows. One-game language when there is no prior game.
- Editorial pass: no takeaway repeated across overview, phase and lesson; a colour before every
  coordinate in every narrated line; Japanese terms glossed; point names checked.

### 12. Validate

```
python <sandbox_dir>/scripts/validate_lesson.py <parsed.json> <lesson.json>
```

Set `schema_version: 2` and `grounding_version: 1`. Fix every PROBLEM line; read the warnings.
The validator checks references and legality, not the meaning of free prose: reread each
checked sentence against facts.json.

### 13. Generate and deliver

```
python <sandbox_dir>/scripts/generate_lesson.py <parsed.json> <lesson.json> <output.html>
```

Open the HTML: praise cards with rating sources, the level line, one narrated sequence per
lesson, puzzle wrong-move playback. Write to `/scratch/work/` first, promote with
`workspace_emit`, then `present_output`. Deliver the single HTML file.

## Run as a team when the platform allows it

`references/orchestration.md` splits the same steps: Stage 0 (steps 1–4 plus a research plan,
sequential); Stage 1 (parallel: one agent per moment for steps 6, 7, 9; an overview agent for 5
and 11; a praise agent for 8; a concepts agent for 10); Stage 2 (`scripts/merge_lesson_parts.py`,
editorial pass, steps 12–13, re-dispatch only the failing part). Slices come from
`scripts/slice_for_agent.py`; each agent gets one prompt template with its slice, forbidden
actions and exact output JSON. The same file gives the single-agent order.

## Board-fact discipline

The student will check your claims against the board. Only state what the report supports:

- **Colours**: every narrated move is a copied `*_with_colours` token; the first mover of a
  refutation is the opponent, of a better line the student — but you never derive this, you copy.
- **Point names**: count from the corner using the coordinates. On 9x9, B3 is the 2-3 point of
  the lower-left corner (column B = 2nd line, row 3 = 3rd line); C3 is the 3-3 point; the
  corner point A1 is 1-1. Write the coordinate next to the name.
- **Captures**: only when `moves[].stones_captured > 0`, `captured_at`, or an
  `arc.status_changes` row says so; state the stone count.
- **Life and death**: status labels are ownership predictions. Unconditional life/death, seki or
  eye counts only when reading or verified board facts establish them.
- **Atari and liberties**: from `facts.lessons[N].groups` or `board_checks`, never inferred.
- **Praise**: only `teaching.praise` entries and recorded captures (step 8).
- **Level**: `student_profile` wording with "plays like" and the caveat; never a rating.
- **Where the points went**: `loss_region`. **Tactics beyond the PV**: your own illustration, or omit.
- **Coordinates**: GTP letters A–T without I, rows from the bottom; check each against the stone lists.

## Tone and style

Write as a patient teacher at the band's register (level-ladder.md). Explain WHY, not just
WHAT ("B D7 connects your stones so White cannot cut at W E7"); concrete language ("one liberty
left — atari", not "ownership −0.9"); encouraging and honest — praise comes from the evidence,
never from goodwill; one memorable habit per lesson.

## Reference files

- `references/orchestration.md` — team stages, one prompt template per agent, single-agent fallback
- `references/level-ladder.md` — rank bands, concept ladder with Go Magic/Sensei's/tsumego tiers, selection rules, prose register, escalation
- `references/evidence-format5.md` — format 5, evidence versions 1 and 2, lesson schema 2
- `references/grounding-and-practice.md` — facts.json, fact_checks, puzzle objectives and verification
- `references/game-arc-commentary.md` — phases, chains, sequential searches, verified Go Magic library
- `references/go-teaching-concepts.md` — themes, praise kinds, Japanese terms, curated anecdotes
- `references/tsumego-source-formats.md` — machine-readable tsumego sources and parsers
- `references/html-build-guide.md`, `references/katago-review-format.md` — HTML behaviour; legacy schema and formats 2–4
