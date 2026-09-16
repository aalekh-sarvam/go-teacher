# Format 5: evidence → explanations → practice

Read this for new reports. References describing formats 2–4 remain for older uploads.

## Evidence versions

The evidence block carries `report_format: 5` and `evidence_version`. The parser accepts
**version 1 and version 2**; any other value is refused and the user is told the skill and
go_teacher are out of step. Version 2 adds the fields below; a version-1 report simply lacks
them (treat them as unknown, never as zero or empty praise).

| Field (version 2) | Where | Use |
|---|---|---|
| `student_profile` | top level | Level statement and band selection (below) |
| `teaching.praise[].kind_label`, `note`, `stones_captured`, `gap`, `second`, `human_prob`, `rating`, `rating_source` | praise entries | The only source of praise (below) |
| `refutation_with_colours`, `better_line_with_colours` | each teaching candidate | Narrating the played and best lines |
| `evidence.sequences[].moves_with_colours` | each sequence | Narrating sampled lines |
| `move_details[n].candidates[].pv_with_colours` | candidate tables | Narrating alternatives |
| `moves[].stones_captured` | timeline | Capture ledger: stones removed by that move |
| `search_coverage` | top level | How much of the shortlist reached the requested deep visits (below) |
| `evaluation_coverage`, `investigation_coverage` | each teaching candidate | Whether the move is deeply verified and whether optional investigations exist (below) |
| `evidence.unavailable.teaching_investigations` | candidate without investigations | Replaces legacy `unavailable.deeper_search`; both mean only "no investigations" |

### `student_profile`

```json
"student_profile": {
  "estimate": {"rank_value": -12.3, "rank_label": "12k", "low_label": "15k", "high_label": "10k",
               "band_stones": 2.4, "samples": 33,
               "wording": "plays like a 12k in this game (range 15k–10k, 33 moves)"},
  "phases": [{"phase": "opening", "estimate": {...}}],
  "working_rank": {"rank_value": -11.6, "rank_label": "12k", "games": 3},
  "profiles_used": {"student": "rank_12k", "target": "rank_8k", "opponent": "rank_20k",
                    "student_source": "working rank from earlier games", "opponent_source": "SGF rank",
                    "small_board_caveat": true},
  "sgf_rank": "15k",
  "caveat": "The estimate is the similarity of this game's moves to human play ... not a rating ..."
}
```

Rules: quote `estimate.wording` verbatim and follow it with the caveat; the phrase is "plays
like", never "is" or "rated". `estimate` and `working_rank` may be null (too few moves, no
history): fall back to `profiles_used.student` and say where it came from (`student_source`).
Phase estimates are absent when the sample was too small; do not compute your own. The band for
lesson and puzzle selection comes from `working_rank.rank_label` first (level-ladder.md).

### Praise entries

```json
{"move_number": 33, "move": "J5", "player": "B", "kind": "Capture", "kind_label": "capture",
 "note": "captured 6 stones without losing points", "stones_captured": 6, "gap": 62.9,
 "second": "H4", "human_prob": 0.18, "rating": "a move most 8k players find first",
 "rating_source": "human-policy ladder (weakest profile whose first choice is this move)"}
```

`kind_label` values and what each means: `capture` (stones removed or an opponent group killed
without losing points), `saved a group` (the student's own group forecast went from dead to
alive), `only good move` (KataGo's first choice, the best other searched move `gap` points
worse), `non-obvious best move` (best move that the student's human profile gives under 25%),
`best move` (first choice with a smaller gap). `note` is a factual sentence written by the
program: copy it. `rating` and `rating_source` are both present or both null; copy both or
neither. `human_prob` is a model probability at the student's profile, not a frequency. A move
absent from `teaching.praise` is praised only when `moves[].stones_captured > 0`, and then only
for the capture. The list is empty when nothing qualified; then say nothing about praise.

### Colour tokens

Every `*_with_colours` list writes one token per ply: `"B D7"`, `"W E3"`, `"W pass"`. The
first token of `refutation_with_colours` is the opponent's; of `better_line_with_colours` the
student's; of `moves_with_colours` whoever `first_to_move` names. Narrate a line by copying the
tokens in order ("W F3, then B D7, then W H8"); never derive colours from move parity and never
write a bare coordinate list. In version 1 these lists are absent: build them from `first_to_move`
with a short script, not by hand, before narrating.

### Search coverage (newer version-2 reports; additive)

The app first evaluates every position, then re-evaluates the positions before and after each
shortlisted mistake at the requested deep visits and keeps searching until the final shortlist is
covered. These fields record what actually happened. They are small and stay in brief.json;
`parse_review.coverage_summary(parsed)` and `facts.coverage[N]` give one line per candidate.

```json
"search_coverage": {"version": 1, "two_pass_enabled": true, "requested_deep_visits": 1000,
                    "final_candidates": 8, "verified_candidates": 8, "verification_rounds": 1,
                    "additional_positions": 4, "reused_positions": 12},
"teaching": {"candidates": [{
  "move_number": 43,
  "evaluation_coverage": {"status": "complete",
    "before": {"turn": 42, "visits": 1000, "requested_visits": 1000, "purpose": "deep", "status": "complete"},
    "after":  {"turn": 43, "visits": 1000, "requested_visits": 1000, "purpose": "verification", "status": "complete"}},
  "investigation_coverage": {"status": "not_selected", "reason": "outside_featured_budget"},
  "evidence": {"unavailable": {"teaching_investigations": "not selected for featured analysis"}}
}]}
```

Two independent questions, two fields:

- **`evaluation_coverage.status`** — was the move *deeply evaluated*? `complete` (both the position
  before and after the move reached the requested deep visits; the only status that supports the
  words "verified" / "deep evaluation"), `incomplete` (at least one side still `pending`; say so, never
  call it verified; a finished two-pass run should not show this), `disabled` (single-pass analysis;
  the base visits are the evidence), `unknown` (not recorded). `before` / `after` carry `turn`,
  `visits`, `requested_visits`, `purpose` (`initial` | `deep` | `verification` | `unknown`) and a
  position `status` (`complete` | `pending` | `disabled` | `unknown`). Quote visits from these fields
  ("verified at 1000 visits on both sides").
- **`investigation_coverage.status`** — do the optional human/local/what-if *investigations* exist?
  `available` (all ran: `evidence.sequences`, `local_reading`, `rollout_comparisons`, `human_refutations`
  are present), `partial` (some; use only the sequence IDs present; `failed` / `inapplicable` list the
  rest), `not_selected` (outside the featured budget; `reason` says why), `disabled`, `unavailable`.
  Anything other than `available` / `partial` means: no `local` or `what_if` panel, no
  `sequence_explanations`, no human reply in prose. It says **nothing** about deep evaluation: a
  `complete` + `not_selected` candidate is a verified mistake taught from its refutation, better line,
  policies and chain.
- `search_coverage`: `verified_candidates` of `final_candidates` are `complete`; `verification_rounds`
  are catch-up batches after the first deep pass; `additional_positions` are positions those batches
  searched; `reused_positions` were already deep. `requested_deep_visits` is null when `two_pass_enabled`
  is false. `interpretation.coverage` repeats the two-status rule.

Legacy rule: reports with `evidence_version` 1, format 2–4, or a version-2 block written before these
fields have **unknown** coverage (`legacy: true` in the summary) — never "incomplete" and never
"verified". Their `evidence.unavailable.deeper_search` ("not selected for featured analysis") means
exactly what `teaching_investigations` means now: no investigations for that candidate. Do not read
either key as "not deeply evaluated". Unknown coverage does not require the app's JSON or a
re-analysis; teach from the evidence present.

## Workflow and data contract

Run `python scripts/parse_review.py report.md parsed.json --brief-output brief.json`.
Read brief.json to select moments and inspect the compact whole-game context; read the complete selected candidates in parsed.json before
narrating sequences. The brief keeps the complete move timeline and summaries, but omits candidate stone lists and truncates sequence previews.
Always pass **parsed.json** to validation and generation. The Markdown is the only input needed.
Its single fenced `go-teacher-evidence` JSON block is authoritative. Reject incomplete blocks.

Top level: `game_info`, `moves`, `move_details`, `teaching`, `summary`, `arc`, `openings`,
`history`, `profiles`, `rank_fit`, `endgame_values`, `missed_opportunities`, `provenance`.
Setup stones and initial player are explicit. Actual moves have explicit B/W colours. Some engine
structures use `Black`/`White`; do not reinterpret them as alternating move numbers.

Each `teaching.candidates[]` contains `move_number`, `player`, `played_move`, `preferred_move`,
`point_loss`, `theme`, the actual stone lists, chains, atari facts, base refutation and an
`evidence` object. The optional investigations (human samples, local reading, trees, rollouts) cover
the featured moments (default four). Other shortlisted moments keep their existing facts and have an
explicit reason in `evidence.unavailable.teaching_investigations` (legacy key: `deeper_search`); see
"Search coverage" above — that reason is about investigations, not about deep evaluation.

`evidence.sequences[]`: stable `id`, `from_turn` (number of actual game moves before branching),
`first_to_move`, `moves` (including any initial played/best move and passes), `source`, `profile`,
`score_black`, `visits`, `stop_reason`. The full application JSON also stores `seed` and `probabilities`
for sampled moves after the fixed first move; these are omitted from the compact report. Every pass consumes a ply and changes player.
A PV score is the search estimate; a sampled rollout score is a separately searched endpoint.
No silent substitution of engine moves for missing human data. Unavailable fields are not zero.

## Compact game context (optional additions to format 5)

Existing format-5 reports remain valid. New reports add fields without replacing any teaching
candidates, alternatives, refutations, human sequences, phase facts or status events.

- `moves[]` still has `number`, `color`, `move`. It now adds `point_loss` for the **mover** and
  `score_black` **after** that move, and in version 2 `stones_captured` (stones removed by that
  move; 0 means none). Positive score favours Black; negative favours White.
  The score before move N is the score after N−1, or `initial_position.score_black` for move 1.
  `initial_position` also includes Black winrate (0–1) and search visits; it can be null.
  Never alternate the score sign with the player to move. For the student's lead, negate
  Black's score only when `game_info.student == "W"`.
- `summary.accuracy.B/W` retains `moves` and `mean_loss`; it adds `total_loss`, `median_loss`,
  `top1_moves`, `top3_moves`, `ranked_moves` and `category_counts`. Loss aggregates clamp negative
  values to zero; the timeline preserves the signed estimate. Totals are not the final margin.
  Top-choice counts are not percentages or strength ratings. Check `ranked_moves` coverage;
  mean/median alone cannot identify the tactical cause of a mistake.
- `summary.timing.status` is `available` if any valid clock-derived time exists, otherwise
  `unavailable`. `players.B/W` each has `timed_moves` and `untimed_moves`. With recorded times it
  adds `median_seconds`, `mean_seconds`, and `at_or_below_median` / `above_median` buckets with
  `moves` and `mean_loss`. The split is **per player**, with ties in the first bucket. Empty
  bucket loss is null. `moves[].time_spent_seconds` is absent when unknown; zero means recorded
  zero, not missing. Sparse samples and differences in position difficulty limit comparisons.
- `context_moves` indexes supporting positions: `opponent_mistakes` (up to six losses of at
  least 2 points, largest first), `checkpoints` (every 25 moves, or 50 when the game exceeds
  200 moves), and `last_move` (null for an empty record). Their compact evaluations and searched
  alternatives are in `move_details[str(number)]`, alongside the existing teaching/praise
  entries. Read a supporting entry from parsed.json when explaining that transition or missed
  opportunity; it is not automatically another student lesson. Entries shared by categories
  are stored once. Only teaching candidates carry the full generated hints, atari facts and
  local chains; do not assume that commentary exists for every context move.
- Continue using the unchanged `arc.phases`, `arc.status_changes`, local chains, atari hints,
  `captured_at` and `missed_opportunities`. Status events are selected detections, not a
  safety assessment of every group; version 2 also records captures of groups the engine
  already counted dead (`from: "dead"`, `to: "captured"`). The complete capture ledger is
  `moves[].stones_captured`. No event does not imply a safe group.

Use this context for a few broad observations, then focus the lesson on specific mistakes and
improvements. For example, a lower middlegame loss can support "your middlegame was steadier
than your opening." To say a lead was sustained or surrendered across a range, check **all**
intermediate scores and both players' losses with a short script, not just the endpoints.
Distinguish the student's losses from opportunities the opponent offered. Report an estimated
lead change, not a guaranteed win; negative move loss can reflect search disagreement.

A repeated-habit claim (such as answering locally instead of taking bigger points) needs multiple
supporting teaching examples. A missed-capture claim needs a capture/refutation or searched
alternative establishing it; a score dip alone cannot. Describe choices and consequences, not
what the student saw, thought or ignored. Use one-game language unless existing history supports
an across-game trend. The timeline is not a set of searched alternatives for every move.

Keep `overall_feedback`, `game_arc`, lesson `story`, praise and practice. Give each a purpose:
overview = main strengths and priority; phases = meaningful changes in balance; lesson story =
local context; lesson panels = move, reply and improvement. Avoid restating every mistake in all
four places. No new compulsory lesson or quiz arises from a summary statistic. Continue choosing
quiz concepts from the selected lessons and their deeper evidence, researching fresh external
positions and checking the transformed solutions as before.

When these optional fields are absent in an older report, use its existing phase/teaching facts;
do not request the JSON or SGF, fabricate evaluations, or treat missing clocks as zero.

## Choosing what to teach

| Evidence | Use in a lesson | Transfer to a researched quiz |
|---|---|---|
| `pass_comparison`, `move_values` | Explain the cost of yielding a turn and compare candidate gains against the same pass baseline. | Find an external urgent-versus-big or endgame ordering example; change the local choice so counting matters. |
| `initiative` | Distinguish a local punishment from an answer to a threat; quote the finite-search estimate and any tested ignoring cost. | Compare an external forcing exchange with a superficially similar move that permits tenuki. |
| `ownership_plan` | Explain which regions the better plan favours. Positive `cost_of_played` is better-minus-played for the named mover. | Transform an external direction-of-play problem, changing surrounding support stones so the intuitive direction is wrong. |
| `difficulty` | Explain whether the search found one narrow choice or several near-best moves; high entropy does not itself prove human difficulty. | Use a plausible decoy from a researched example; require an extra reading step to distinguish it. |
| `local_reading` | Show attacker-first and defender-first restricted lines, allowed region and finite depth. Describe favourable/unfavourable ownership predictions. | Research a related tsumego or capturing race. Independently verify the new puzzle's life/death claim; the game's ownership value cannot certify it. |
| `tree` | Compare evaluated root choices and reply branches; each node's loss belongs to the player making that node's move. | Build an external reading puzzle with two plausible replies; show why the best defence still fails after the wrong move. |
| `human_refutations` | Show the opponent profile's most likely reply and its engine evaluation. It can miss the punishment. | Make a tempting wrong choice human-plausible, then verify its refutation on the new board. |
| `sequences`, `rollout_comparisons` | Switch between the engine and two student profiles; explain how subsequent choices differ in one sampled future. | Test whether the student can follow the principle after the first correct move in a different externally sourced position. |
| `missed_opportunities` | Explain an opponent's gift and how the student gave it back. Do not infer a local tactic from two score losses alone. | Adapt an external punish-the-mistake problem with an unobvious continuation. |
| `rank_fit`, `student_profile` | Quote `student_profile.estimate.wording` with its caveat; it is similarity to human play, not a calibrated rank. | Choose the band, puzzle tier and prose register from level-ladder.md. Do not invent rank-rated puzzles; state the tier's source instead. |
| `teaching.praise` | The only source of praised moves: copy `note`, `rating`, `rating_source`; explain non-obviousness from `human_prob`. | None (praise is not practised). |

Use the evidence that supports each chosen lesson. Do not force every available field into every
lesson or quiz. Preserve the game arc, cause/effect story, constructive praise, principles and
resources. Research has **no search-count, source-count or time cap**; continue as needed to find
and verify suitable explanations and examples. Go Magic remains the first resource to consider.

## Claims that the data does not establish

- Winrate saturation does not mean a beginner game was decided. Point loss still matters and
  later lead reversals must remain in the arc. `engine_strongly_favoured_side_before` is literal.
- Ownership, including legacy `status_changes` labels, predicts ownership under subsequent play.
  It does not prove unconditional life, death, seki, two eyes or a specific killing tactic.
- A nearby reply does not prove sente. `initiative.class` is an estimate; a punished mistake
  must not be praised as a forcing move. Unknown tenuki cost is not a lower bound.
- Pass comparisons are whole-board values against a particular searched baseline. Do not call
  them exact local endgame values, mathematical temperature, miai value or deiri value.
- Sampled future differences contain later mistakes and randomness. Do not call them a causal
  estimate of the original move or alter mistake severity using opponent strength.
- The human policy likelihood is a probability under a model/profile, not a survey frequency.
  Phase labels are move-count heuristics. Opening patterns are geometric hints, not joseki IDs.
- Existing history is descriptive context; do not add cross-game pattern mining or a style vector.
- Clock time alone cannot distinguish a knowledge gap from poor discipline.

## Lesson schema 2 (author prose, reference evidence)

For new lessons also set `grounding_version: 1` and follow [grounding-and-practice.md](grounding-and-practice.md). It defines fact tokens/checks, current statistics, sequence-specific explanations and stronger puzzle verification. The base schema below remains supported for older authored lessons.

```json
{
  "schema_version": 2,
  "game_title": "A useful habit from this game",
  "overall_feedback": "Two connected paragraphs grounded in the report.",
  "game_arc": {"intro": "The actual game arc.", "phases": [
    {"phase": "Opening", "move_range": "1–20", "narrative": "What happened."}
  ]},
  "lessons": [{
    "move_number": 23,
    "title": "Count the reply before moving",
    "concept_label": "Reading",
    "theme": "reading / tactics",
    "story": "How this position arose, using actual move numbers.",
    "principle": "Before playing elsewhere, read the opponent's strongest local reply.",
    "level_framing": "The 12k profile gives the played move about 1 in 1000; the 8k profile prefers H4 (78%).",
    "panels": {
      "position": "What to notice before moving.",
      "played": "White plays W H4, Black answers B J5, then W G9: walk the copied colour tokens and say what the move allows.",
      "best": "Explain the stronger plan, walking better_line_with_colours the same way.",
      "alternatives": "Explain how to compare the searched branches.",
      "local": "Explain the restricted reading and its limitations, when available.",
      "what_if": "Explain what the two sampled futures illustrate."
    },
    "alternative_explanations": {"D4": "Prose about this searched choice, if present."}
  }],
  "puzzles": [],
  "good_moves": [{"move_number": 33, "move": "J5", "kind_label": "capture",
                  "explanation": "captured 6 stones without losing points. ...",
                  "rating": "copied", "rating_source": "copied"}],
  "concepts_learned": []
}
```

Use actual move numbers and searched alternatives; the example is schematic. In schema 2,
**do not emit** `alternatives`, `played_quality`, `concept`, played/preferred moves, colour,
point losses, winrates, arrays, trees, refutations or ownership in the lesson object: the
generator rejects the forbidden keys and joins the data from parsed.json. Keep `panels` values as
prose strings built from copied colour tokens; the generator displays the numerical support and
the sequences itself. Exact line commentary must be grounded in the full sequence, not the brief
preview. `good_moves` entries come only from `teaching.praise` (or recorded captures); see
SKILL.md step 8. Legacy schema 1 remains supported for format 2–4 reports. Old generator fields
for game_arc, story, concepts, praise and progress remain.

## Practice schema additions

Each schema-2 puzzle uses the existing `title`, `concept_label`, `board_size`, `black_stones`,
`white_stones`, `player_to_move`, `opponent_last_move`, `correct_moves`, `hint`, `explanation`,
`wrong_moves`, `generic_wrong_explanation` plus:

- `lesson_move_number`: reference the lesson whose idea transfers.
- `evidence_focus`: one field from the table above (use `rollout_comparisons` for sampled futures).
- `transfer_explanation`: why this new position tests the same principle.
- `source`: `{ "url": "https://...", "title": "The external example actually consulted" }`. Grounded lessons also require `example_locator` and the original `position`, as described in grounding-and-practice.md.
- `transformation`: describe meaningful changes to that external example. Rotation alone does
  not make a puzzle non-obvious; change surrounding support, liberties or the tempting response
  and re-check the resulting solution.
- `verification`: state how the new answer and strongest defences were checked, and limitations.
- `correct_lines`: map each correct move to a complete legal sequence **starting with that move**.
- Each wrong choice has a legal `refutation` starting with the opponent's reply after that wrong
  move, and an explanation. Use qualitative grading when no evaluation of the puzzle exists.
  A numerical `loss_vs_best` additionally requires `evaluation_source` for this puzzle; never copy
  game losses onto a different position.

Never use an actual game position, its symmetry/colour swap, or a trivial edit as a quiz. The
validator rejects equivalent board shapes but cannot establish that a novel puzzle is sound.
Validate the strongest defence through an external solution, actual engine search when available,
or explicit careful reading. Do not claim the legality validator proved the answer best.

Run `validate_lesson.py parsed.json lesson.json`, then `generate_lesson.py parsed.json lesson.json lesson.html`.
The generator also validates before writing. Open the offline HTML: test all three perspectives,
sequences, passes and puzzle wrong-move playback. Do not deliver placeholders as a lesson.
