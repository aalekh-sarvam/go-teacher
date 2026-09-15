# Format 5: evidence → explanations → practice

Read this for new reports. References describing formats 2–4 remain for older uploads.

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
`evidence` object. Deeper computations cover the featured moments (default four). Other
shortlisted moments keep their existing facts and have an explicit unavailable reason.

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
  `score_black` **after** that move. Positive score favours Black; negative favours White.
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
  `captured_at` and `missed_opportunities`. Status events are selected detections, not a complete
  capture ledger or a safety assessment of every group. No event does not imply a safe group.

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
| `rank_fit` | Describe relative similarity to tested profiles in each phase, cautiously. It is not a calibrated rank. | Adjust explanation and scaffolding, not truth, from the student's chosen level and observed mistake. Do not invent rank-rated puzzles. |

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
    "story": "How this position arose, using actual move numbers.",
    "principle": "Before playing elsewhere, read the opponent's strongest local reply.",
    "panels": {
      "position": "What to notice before moving.",
      "played": "Explain the consequence without claiming the sampled human reply is certain.",
      "best": "Explain the stronger plan.",
      "alternatives": "Explain how to compare the searched branches.",
      "local": "Explain the restricted reading and its limitations, when available.",
      "what_if": "Explain what the two sampled futures illustrate."
    },
    "alternative_explanations": {"D4": "Prose about this searched choice, if present."}
  }],
  "puzzles": [], "good_moves": [], "concepts_learned": []
}
```

Use actual move numbers and searched alternatives; the example is schematic. In schema 2,
**do not emit** played/preferred moves, colour, point losses, winrates, arrays, trees, refutations
or ownership in the lesson object. The generator joins them from parsed.json. Keep `panels`
values as prose strings. It displays the numerical support itself. Exact line commentary must
be grounded in the full sequence, not the brief preview. Legacy schema 1 remains supported for
format 2–4 reports. Old generator fields for game_arc, story, concepts, praise and progress remain.

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
