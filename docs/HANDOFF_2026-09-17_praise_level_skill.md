# Handoff: finish the praise / level-tailoring / parallel-skill work

**Status 2026-09-17 (later):** all items in this file, the 14 suggestions and the coverage handoff
(`docs/TEACHING_SEARCH_COVERAGE_HANDOFF.md`, sections 1–15) are implemented; see
`docs/VALIDATION_2026-09-17.md` for the verification run and the acceptance checklist. The text
below is kept as the record of what was planned.

Written 2026-09-17 for an agent taking over. Read this file, then `docs/PLAN_2026-09-17_level_praise_parallel.md`
(the plan the user approved; its two lists are reproduced at the end of this file), then the code
pointers below. Everything else in `docs/` is background; do not use `docs/TEACHING_SEARCH_COVERAGE_HANDOFF.md`
as an input (the user set it aside).

**The teaching skill is executed by GLM 5.3**: capable, not frontier. Every skill change must
favour small per-agent inputs, fixed JSON schemas, facts supplied as data rather than transcribed
from prose, and validators that fail loudly. Do not add free-form "figure it out" steps.

## Basis for all decisions

Only `evals/20260917_evals/` counts as evidence of what went wrong:
- the game `KaTrain_Human (Normal Game) vs AI (Human-like) 2026-09-16 20 08 45.sgf` (9x9, student
  Black, 66 moves incl. two passes, B+74.5, opponent a 20k KaTrain bot);
- the app outputs for it: `…20.md` (main report, format 5), `…20.detailed.md`, `…20.json`, `progress.json`;
- the skill used: `go-game-teacher.zip` (now unpacked as the working copy at `skill_indus/go-game-teacher/`);
- the lesson it produced: `Go_Lesson_Human_vs_AI_9x9.html` (the only lesson to look at).

Facts established (verified against those files):
- Student captures of 6 stones (move 33 J5), 5 (move 55 C1) and 8 (move 59 J2) appeared nowhere; the
  lesson wrote "nothing else changed hands" in the endgame.
- Praise was empty. Old rule: rank #1, Black winrate 5–95% before the move, second candidate ≥5
  visits, gap ≥2 points, max 3. Winrate left the band at move 12; moves 31/33 had 1-visit second
  candidates. The detailed report printed "No move by the student stood out as the only good move in
  a live position."
- "Your level · 20k" was the upload default (equal to the *opponent's* SGF rank), not measured.
- The lesson invented praise ("B3 at move 49 was praised by the engine"), swapped colours when
  narrating engine lines, called B3 a "2-1 point" (it is 2-3), and its history disagreed with the
  report because `progress.json` holds the same game four times (`file_stem` empty).
- `difficulty` labelled 1000-visit positions with 98% of visits on one move "insufficient search".

## State at this checkpoint (commit "Checkpoint: praise rewrite, capture ledger, rank ladder…")

`cargo build --release` and `cargo test` (17 passed, 1 ignored) are green. Done in the app:

| Area | Where | What |
|---|---|---|
| Capture ledger | `src/analysis.rs` `build_reviews` | `MoveReview.stones_captured` from the board replay (`teaching::boards`). |
| Status changes | `src/teaching.rs` `status_changes` | Captures of groups the engine already counted dead are recorded (`from: "dead"`, `to: "captured"`). |
| Praise rewrite | `src/teaching.rs` `praise(reviews, changes, student, max)` | No winrate window. Gap = played score minus the best *other* searched move by score, any visit count. `PraiseKind` {OnlyMove, Capture, Save, NonObvious, Steady}, priority capture > save > only move > non-obvious > steady; relaxed thresholds when fewer than 3 qualify; max 5; `note`, `stones_captured`, `human_prob`, `rating`, `rating_source` fields. |
| Deep pass | `src/analysis.rs` `deep_turns(.., praise, ..)` | Preliminary praise positions (before/after) are re-analysed at deep visits. |
| Theme rules | `src/teaching.rs` `build` | Own group hurt → life and death first; a capture or atari that still lost points → "priority (played locally, bigger move elsewhere)". |
| Rank ladder | `src/probes.rs` `rank_fit` | 11 profiles (20k…3d) over all student non-pass moves (stride-sampled to ≤60); tempered likelihood-weighted `estimate` {rank_value, rank_label, low/high, band_stones, samples, wording} overall and per phase; `move_ranks[]` with `weakest_profile_choosing_it_first`; helpers `rank_value`, `rank_label`, `profile_for_value`. |
| Praise rating | `src/analysis.rs` after `probes::analyze` | `rating` = "a move most 8k players find first" (or "not the first choice of any tested human profile"), `rating_source` set. |
| Difficulty | `src/probes.rs` `difficulty` | Labels: "under-searched" (<100 visits), "one clear move" (top share ≥0.9 and gap ≥2, or a single candidate), "narrow choice…", "several searched choices"; `top_visit_share` added. |

Not yet done (in order), with exact pointers:

### 1. Surface the new data in the report and evidence (app)

- `src/evidence.rs` `build`: (a) add `stones_captured` to each `moves[]` row; (b) `teaching.praise[]`
  already serialises `PraiseCandidate` — verify the new fields appear (kind, note, rating,
  rating_source); (c) add `student_profile`: `{ "estimate": rank_fit.estimate, "phases": [...],
  "upload_profile": profiles.student, "target_profile": profiles.target, "opponent_profile":
  profiles.opponent, "working_rank": <from progress, see 3>, "caveat": "..." }`; (d) for every
  sequence in `teaching.candidates[].evidence.sequences[]` and for `refutation` / `better_line`, add
  `moves_with_colours: ["B D7","W E3",…]` (alternate from `first_to_move`; a pass keeps the colour
  sequence: "W pass"). Also add it to `move_details[].candidates[].pv`.
- `src/report.rs` `render_markdown` (main report): add `## Student profile` (the `wording` line,
  per-phase labels, the profiles used, working-rank trend, caveat) and `## Good moves` (table: move,
  kind, note, gap, rating). Never print "No move stood out…" when `stones_captured > 0` anywhere; the
  detailed report's praise section (`render_detailed_markdown`, "Good moves worth praising") should
  print kind and note per row.
- `src/report.rs` detailed turning points (search "### Turning points"): keep an entry only when both
  turns are in `a.deepened` (or `a.deepened` is empty) or `|point_loss| >= 3.0`.
- Bump `evidence_version` to 2 in `evidence.rs` (keep `report_format` 5); the skill parser must accept 1 and 2.
- Fake-engine test (`src/analysis.rs` tests): add a canned 6-stone capture at 99% winrate in
  `tests/fake_katago.py` and assert a `Capture` praise with a rating; assert `student_profile` and
  `Good moves` appear in `render_markdown`.

### 2. Profiles driven by the estimate (app)

- `src/probes.rs` profile resolution (search `out.profiles = json!`): student profile = upload
  override, else the student's SGF rank, else the **working rank** (see 3) rounded via
  `profile_for_value`, else `rank_10k`; target = two or three stones stronger (`stronger` twice).
  Record `student_source` in `profiles`.
- Because rank fit runs at the end, either run a cheap ladder pass before the moments (preferred: move
  the `rank_fit` call ahead of the moment probes; it is ~1 s per profile on a 9x9) or keep the upload
  default for this game and let the working rank apply to the next game. Prefer the former.

### 3. Progress history (app, `src/progress.rs`)

- Deduplicate by game identity: (date, player names, move count, result, board). Keep the newest
  analysis of a game; `history_for` must exclude older analyses of the same game.
- Fill `file_stem` from the SGF file name (pass it into `entry_for`/`record` from `server.rs`
  `finish()` and `main.rs analyze_files`).
- Add `rank_estimate: Option<f64>` (the overall `rank_value`) to `GameEntry`; add
  `working_rank(out_dir, a) -> Option<f64>`: exponentially weighted mean (alpha 0.5) over the last
  five distinct games' estimates including this one. Surface in `student_profile` and in the
  "Compared with earlier games" section.

### 4. Skill: make it use the new evidence (working copy `skill_indus/go-game-teacher/`)

Run `python3 -m unittest discover -s tests` before and after every change (26 tests pass now).

- `scripts/parse_review.py`: accept `evidence_version` 1 and 2; expose `student_profile`,
  `moves[].stones_captured`, `moves_with_colours`, praise kinds. Add `--moves N…` passthrough to
  `teaching_facts.build_facts` so facts.json is per-moment (currently 113 KB for everything).
- New `scripts/slice_for_agent.py parsed.json --moment N | --role overview|praise|concepts` → a
  10–20 KB JSON slice per sub-agent (candidate evidence + move_details[N] + facts.lessons[N] +
  profiles + student_profile + arc). This is the main speed and hallucination lever for GLM 5.3.
- New `scripts/merge_lesson_parts.py part1.json part2.json … -o lesson.json`: assembles agent outputs,
  renumbers `fact_checks[].text_path` lesson indices, dedupes `concepts_learned.resources`, then runs
  `validate_lesson.py`.
- `references/orchestration.md` (new): the stage plan — sequential: parse, identify student, choose
  moments (2–3, distinct themes, at or one band above the working rank, weakest phase), research plan;
  parallel: one agent per moment (research + prose + fact checks + its puzzle), overview/arc/progress
  agent, praise agent, concepts agent; sequential: merge, editorial pass (no repeated takeaway),
  validate, generate, re-dispatch only the failing part. Give one prompt template per agent with its
  input slice and output schema. Add a short "Run as a team when the platform allows it" section to
  SKILL.md; keep the numbered single-agent steps.
- `references/level-ladder.md` (new): concept → rank band → Go Magic course/lesson (from the verified
  library in `game-arc-commentary.md`) → Sensei's page → tsumego tier (Tasuki cho-1 elementary for
  20k–10k, cho-2 intermediate 10k–5k, cho-3 advanced above; OGS collections by `puzzle_rank`).
  Selection rule and prose register rules from the plan (A1.2).
- Praise agent: SKILL.md step 5a becomes "3–4 `good_moves` from `teaching.praise` (now populated),
  each stating what it achieved (stones captured, group saved, gap), why a beginner might miss it
  (`human_prob`), the `rating` with `rating_source` copied verbatim". `check_grounding.py`: require
  `good_moves[].move_number` ∈ praise or `moves[]` with `stones_captured>0` / gain, and that quoted
  numbers match.
- Colours in sequences: every place SKILL.md tells the model to narrate a line, tell it to copy
  `moves_with_colours` tokens; `generate_lesson.py`/`lesson_panels.js`: a single `renderSequence()`
  that prints "B D7 W E3 …" in the panel text; panel prose through the same `**bold**` formatter as
  static prose (it is inserted as `textContent` today).
- Puzzle sources: `scripts/parse_sgf_problem.py` (root `AB`/`AW`/`PL` + first variation as the
  correct line) and `scripts/parse_ogs_puzzle.py` for `https://online-go.com/api/v1/puzzles/<id>`
  (fields: `puzzle.initial_state.black/white` as letter-pair coordinate strings, `puzzle.initial_player`,
  `puzzle.move_tree` with `correct_answer`/`wrong_answer` flags and `branches`, `puzzle.puzzle_rank`,
  `puzzle.puzzle_type`, `width`/`height`, `collection.id`, `private`). Confirm the licence/terms on
  OGS before shipping; both emit the puzzle dict that `check_puzzle` validates. Keep `parse_tasuki_tex.py`.
- Validator parity: add to `validate_lesson.py` every key the generator dereferences (puzzle `title`,
  `concept_label`, `hint`, `player_to_move`; `good_moves[].explanation`; `concepts_learned[].term/
  description`, `resources[].title/url`).
- Docs: fix `references/html-build-guide.md` (still describes the old six buttons and per-alternative
  buttons), remove dead `alt_buttons` code in `generate_lesson.py`, remove `alternatives`,
  `played_quality`, `concept` from SKILL.md step 5 (schema 2 forbids or ignores them).
- Bump the skill's expected evidence versions and document `student_profile`, praise kinds and
  `moves_with_colours` in `references/evidence-format5.md`.

### 5. Verify on the eval game, then package

- App: `./target/release/go_teacher --out-dir <tmp> analyze "evals/20260917_evals/KaTrain_Human (Normal Game) vs AI (Human-like) 2026-09-16 20 08 45.sgf" --two-pass --human-profile rank_20k --human-profile-target rank_15k`
  (about 10 minutes with the probes). Acceptance: moves 33, 31 and 59 praised with kinds capture /
  only move / capture and ratings; `stones_captured` 6/5/8 on moves 33/55/59; `student_profile`
  present with a wording line; no "No move stood out" sentence; turning points list no longer contains
  move 2; theme of move 37 is "priority…", of move 41 "life and death".
- Skill: `parse_review.py` on the new report → `slice_for_agent.py --moment 19` → author a lesson
  JSON (by hand or with the model) → `validate_lesson.py` → `generate_lesson.py`; open the HTML and
  check: praise cards with ratings and sources, colours in every narrated sequence, the level label
  from `student_profile`, puzzles from the matching tier with sources.
- Package: `cd skill_indus && rm -f go-game-teacher-6.zip && (cd go-game-teacher && zip -qr ../go-game-teacher-6.zip SKILL.md references scripts tests)`.
  Rebuild the app with `./packaging/make_app.sh`. Commit and push.

## Working notes for the next agent

- Rust toolchain: `export PATH="$HOME/.cargo/bin:$PATH"`. Quick functional runs: `--visits 30`.
- The fake engine (`tests/fake_katago.py`) understands `allowMoves`, `includeMovesOwnership`, passes,
  `initialPlayer`, per-profile `humanPolicy`; extend it rather than skipping tests.
- All scores/ownership are Black-view; `sign(color)` converts. `Color` serialises as "Black"/"White" in
  most objects but as letters "B"/"W" in `moves[]`, `game_info.student` and `move_details.player`.
- Do not change KataGo thread settings or the engine resolution order.
- The user wants encouragement: 3–4 praised moves per lesson, each with a grounded rating.

---

## The two lists from the plan (verbatim)

### List A: plan for the four observations

**A1 Tailor lessons to the player's capacity.**
1. Judge rank cheaply with the human-style network: eleven profiles at one visit over all student
   positions (≈35 s on 9x9), likelihood-weighted estimate with a band per phase, smoothed into a
   working rank across games; SGF rank as prior; the working rank drives the human profiles and a
   new Student profile section (done in the app except the section and the working rank).
2. Concept ladder mapping themes to rank bands with Go Magic / Sensei's / tsumego tiers; lessons at
   or one band above the working rank in the weakest phase; puzzles from the matching tier with one
   tier-up puzzle when warranted; prose register rules below 10k.
3. Sources: Tasuki collections (parser exists), OGS puzzle API (new parser, check terms), Sensei's
   exercises (SGF parser), Go Magic for framing; GoProblems skipped.
4. Escalation: the working rank moves the band and the tier; a theme handled well in two of the last
   three games moves its puzzles up a tier; a recurring theme holds the tier and rotates collections.

**A2 Parallel sub-agent workflow.** Sequential: parse, student, moment selection, research plan.
Parallel: per-moment agents (research + prose + fact checks + puzzle), overview/arc, praise, concepts.
Sequential: merge with path renumbering, editorial coherence pass, validate, generate, re-dispatch
failing parts. Delivered as orchestration reference + slicing script + merge script.

**A3 Praise.** App: no winrate window, gap against all other candidates, kinds capture/save/only/
non-obvious/steady, deep data, up to five, rating from the profile ladder with a source, capture
ledger, visible Good moves section. Skill: praise sub-agent writing 3–4 grounded examples.

**A4 "No obvious move".** Root cause = the old praise filter plus shallow-pass judgement plus the
difficulty label; fixes = A3, corrected labels, deep-only turning points, theme rules.

### List B: the 14 independent suggestions

1. Write every sequence with colours in the report ("B D7 W E3 …") and render them the same way in
   the lesson; bare coordinate lists caused the colour swaps.
2. Use the SGF undo branches as evidence of what the student considered (the eval SGF shows D3
   tried before F1, and D1/H1 before H4); record "also considered" per move.
3. Decided-game framing: when the winrate has been outside 5–95% for 20+ moves, express losses
   relative to the margin and say the game was won by move 12; later "blunders" are about how much.
4. Deduplicate `progress.json` by game identity and fill `file_stem`; report and lesson currently
   disagree on the history.
5. Validator parity with the generator: check every key the generator dereferences so "0 problems"
   means the HTML builds.
6. Fix stale UI documentation (old buttons), remove dead `alt_buttons` code, stop asking for fields
   schema 2 forbids (`alternatives`, `played_quality`, `concept`).
7. Panel prose is inserted as text, so `**bold**` shows literally; route it through the formatter.
8. Non-featured candidates carry `unavailable.deeper_search` yet have refutations and better lines;
   tell the skill they are valid lesson targets with reduced panels, or feature all candidates.
9. Puzzle/lesson theme consistency check: warn when a puzzle's focus or concept label does not match
   the lesson candidate's theme (the eval taught an outside hane as "nakade").
10. Per-moment facts output (`--moves`) from `parse_review.py` so facts.json is not 113 KB.
11. Jargon glossary auto-glossed on first use for players below 10k (semeai, nakade, kyūsho, "gote-like").
12. Progress card numbers must come from the same deduplicated source as the report's history.
13. Suppress phase rank estimates below the sample threshold instead of printing them with a flag
    (the endgame showed 3d with too few moves).
14. Limit anecdotes and proverbs to the curated list in the concepts reference; the eval produced a
    garbled romanisation and unsupported attributions.
