# Ground explanations and verify transferred practice

Read this for new format-5 lessons. The user still uploads only the main Markdown report.
`facts.json`, authored `lesson.json` and any puzzle search files are working files made by the skill.
Keep the established teaching sections and the report's detailed hints. Most attention goes to
mistakes, replies, alternatives and practice; the overview supplies a few useful observations.

## Read facts before writing

Parse with `--facts-output facts.json`, or after selecting moments run:

```
python scripts/teaching_facts.py parsed.json facts.json --moves 5 17 23
```

Use the actual chosen move numbers. Read `current`, `history` and the selected lessons' short
facts first. Read full `sequences[id]` only for branches you discuss. They explicitly label each
ply's player and distinguish the variation from `actual_moves`. Local defender-first lines can
start with an artificial pass by the attacker: that pass also changes the player.

- `current.accuracy` and `current.phases` are authoritative for this analysis. Missing values
  remain unknown. Top-1 percentage uses all the student's moves as its denominator, with ranked
  coverage retained. Never use a history entry as the current record.
- `history.prior_games` excludes metadata matches to the current game and collapses repeated
  metadata matches among earlier records. Without move fingerprints these are **possible**
  duplicate analyses, not proven identities. `possible_same_game_snapshots` and unidentified
  entries are preserved separately; raw history is untouched in parsed.json. Do not count these
  as additional games. No new cross-game pattern mining is needed.
- `policy` binds each probability to the **played move**, not the recommended move. Older
  target values may be rounded values extracted from a generated hint. A target value of 3.7%
  exceeds 2.7%; it is not “rarer”. Describe probabilities under a model/profile, not percentages
  of people observed to play a move.
- `coordinates` gives exact board orientation and rows. `groups` replays liberties before and
  after the played/preferred moves. An unchanged liberty set cannot itself establish a vital
  local killing move. A remote ownership change is a forecast, not an explanation of its cause.
- `local_trials` names the defender, first local player and ownership **for that defender**.
  Positive/favourable is favourable for that group’s owner, regardless of the student’s side.
- `rollouts[level].favours_start` interprets the signed comparison for the student. A negative
  best-minus-played result favours the played-start sample. It still includes subsequent choices
  and cannot replace the main engine assessment or prove the first move caused the difference.
- `coverage` (also `lessons[N].coverage`) has `evaluation_status`, `before_visits`,
  `after_visits`, `investigation_status`, `legacy` and a quotable `text`
  (`{{fact:/lessons/23/coverage/text}}`). Coverage constrains claims, it does not make them:
  "verified", "deeply verified" or "deep evaluation" is allowed only when `evaluation_status`
  is `complete` (the validator reports a PROBLEM otherwise, including for `unknown` — every
  older report); `incomplete` must stay visible as uncertainty; `unknown` is "coverage not
  recorded", not a defect. When `investigation_status` is `not_selected` / `disabled` /
  `unavailable` (legacy `evidence.unavailable.deeper_search` means the same), there are no
  human, restricted-reading or what-if sequences for that move: omit `panels.local`,
  `panels.what_if` and `sequence_explanations`, and do not describe a "human reply" — the
  validator warns. A `complete` candidate with no investigations is still a fully verified
  lesson built from refutation, better line, policies, chain and searched alternatives. Game
  coverage never verifies a transformed puzzle: puzzle checks stay independent.

## Check the actual draft

New lessons use `schema_version: 2` and **`grounding_version: 1`**. Older drafts still render with
an explicit legacy-validation warning. Keep the prose fields from evidence-format5.md.

For fragile quantitative assertions, insert deterministic text where it reads naturally:

```
"what_if": "{{fact:/lessons/23/rollouts/student/text}} Compare where the two lines diverge."
```

The generator expands the token from the report. Use an existing scalar or text path from
facts.json, never guess an ID. Do not insert large objects into prose. Surrounding interpretation
must still agree with the inserted fact.

Also add top-level `fact_checks`, linking the **actual drafted sentence** to the fact it claims:

```json
"fact_checks": [{
  "text_path": "/lessons/0/panels/local",
  "quote": "White is the defender.",
  "fact": "/lessons/17/local_trials/defender_first/defender",
  "expected": "W"
}]
```

This example is schematic: select facts from the supplied game. Include checks for every
sequence actor, policy attribution/comparison, sampled-result sign, location, group status and
current statistic you assert. One checked sentence is not permission to leave other numerical
claims unchecked. At least one check per selected lesson is required by the validator. Each
quote must occur in the indicated prose, and expected values must equal the derived facts.
Use multiple checks when one sentence makes multiple claims. Read each checked sentence again
against its values: the checker verifies assertions and references, **not the semantic meaning
of unrestricted natural language**. Correct or qualify unsupported causality; a generic caveat
elsewhere does not repair a reversed sign or defender in the sentence itself.

For progress, author the summary prose but let the renderer populate rows from
`current.progress_rows`; grounded lessons ignore manually transcribed progress rows. If there
is no identifiable earlier game, describe this game alone.

Praise (`good_moves`) comes only from `teaching.praise` (evidence version 2): copy `note` as
the first sentence, quote `stones_captured` and `gap` as given, explain non-obviousness only from
`human_prob`, and copy `rating` + `rating_source` verbatim or omit both. The checker rejects a
`good_moves` entry whose move is not the student's actual move, or that carries a `rating`
without `rating_source`; it also requires the move to be in `teaching.praise` or to have
`moves[].stones_captured > 0`. Low loss alone does not prove a move was the only good choice;
the eval lesson's "praised by the engine" claim about a move outside the (empty) praise list is
the error this rule prevents.

Sequence prose (`panels.played`, `panels.best`, `sequence_explanations`) walks the copied
`*_with_colours` tokens in order; `facts.lessons[N].sequences[id].text` gives the same plies as
"1 W F3 → 2 B D7 → …" for fact checks.

Use optional `sequence_explanations: {"played_student": "...", "best_target": "..."}` for line
commentary tied to a particular human sample. Only IDs present in that lesson’s full evidence
are accepted. Include significant moves by both sides. The UI uses this explanation when that
sequence is selected. Without it, the UI explicitly labels the shared engine/general explanation.

## Research and transform a particular external example

Research remains uncapped. Find an accessible **specific diagram/problem**, not just a course
homepage or generic strategy article, from the tier that matches the student's band
(`references/level-ladder.md` table 2): Tasuki `cho-1` / `cho-2` / `cho-3` via
`scripts/parse_tasuki_tex.py`, OGS collections by `puzzle_rank` via `scripts/parse_ogs_puzzle.py`,
Sensei's or gogameguru SGFs via `scripts/parse_sgf_problem.py`. All three emit the same puzzle
dict; verify bounded objectives mechanically with `scripts/solve_tsumego.py`
(`references/tsumego-source-formats.md`). Prefer these over transcribing image diagrams or
hand-reading a solution. Preserve `source.url` and `source.title`; add:

- `source.example_locator`: problem ID, figure number, section/diagram caption or video timestamp.
- `source.position`: `{board_size, black_stones, white_stones, player_to_move}` for that example.
- `transformation`: the meaningful changes to **that external example** and how they change the
  reading while preserving the lesson. The validator rejects symmetry/colour/translation alone;
  you must assess whether adding stones actually changes the decision.
- `solution_review.non_obvious_reason`: the tempting response and the extra reading or comparison
  needed to reject it. A direct atari extension alone is insufficient transfer practice.

The game and trivial edits of its positions remain prohibited. The source diagram is a research
record, not proof that your changed board has the same solution. Do not invent inaccessible
examples, solution coverage or engine runs. Continue researching when an example is unsuitable.

## State and verify the exercise objective

Every new puzzle has `objective: {kind, description, ...}`. Display a concrete question, such as
“capture the marked group within five plies” or “compare the two directions with these support
stones”. Saving a stone for a few moves is different from unconditional life or best whole-board
play. Do not assert `unique_best`: finite KataGo search is evidence, not proof of uniqueness.
Accept equally good verified answers. Unlisted clicks are labelled unverified in the HTML.

Retain `correct_lines`, `wrong_moves[].refutation` and explanatory text. Read beyond the first
plausible reply. There is no fixed line-length limit: include the defender’s best resistance and
the attacker’s follow-up until the claimed consequence is established. A group reaching two
liberties is not proof that it escapes. Passes alternate players just like placements.

Add these review records:

```json
"objective": {
  "kind": "capture_within", "description": "Black: capture the marked White group within five plies.",
  "attacker": "B", "target": "D1", "plies": 5
},
"solution_review": {
  "method": "exhaustive_capture",
  "non_obvious_reason": "Describe the actual decoy and extra reading step on this board.",
  "limitations": "Capture within the stated horizon; no claim about whole-board optimality.",
  "strongest_defences": [{
    "moves": ["G5", "B1", "B2", "A1", "A2"],
    "why_strongest": "Explain the best resistance, including other plausible replies checked.",
    "conclusion": "State the actual verified endpoint, not an assumed escape."
  }]
},
"board_checks": [{
  "after": [], "anchor": "D1", "expected": {"color": "W", "liberty_count": 1}
}]
```

These are illustrative fields, **not a ready-to-use puzzle or a suggested answer label**.
Each offered correct and wrong choice needs a `strongest_defences` line starting with that choice.
Its full line must match the continuation shown by `correct_lines` or the wrong-move refutation.
`board_checks` uses a replayed `after` line and an anchor; expected fields can include `present`,
`color`, sorted `stones`, sorted `liberties`, or `liberty_count`. Check all such claims in the
explanations, including after wrong choices, not only at the initial board.

- `capture_within`: the player is the attacker. `target` identifies an original enemy group;
  `plies` includes the first offered move. The validator searches **all legal replies** to check
  whether each offered choice can force capture within that horizon.
- `avoid_capture_for`: the player is the defender, with the enemy as `attacker`; use the same
  target/horizon fields. It proves only survival for that horizon, never unconditional life.
- `compare_plans`: for strategic/opening/life-and-death lessons that need richer reading. Supply
  `solution_review.method` as `engine`, `external_solution`, or `reading`; document the strongest
  defences, endpoints and remaining uncertainty. Use a verified exact transformed solution or
  sufficiently complete reading. If you cannot establish the answer, redesign or research more.

The bounded capture search has a work limit to keep validation usable; an exhausted search
returns **inconclusive**, which fails validation. This is not a cap on research. Simplify the
exercise without making its decision trivial, or use a separately verified strategic exercise;
do not change its goal merely to hide a failed answer. No legality check certifies difficulty.

If local KataGo is available, extract the authored puzzle to `puzzle.json` and run:

```
python scripts/verify_puzzle_engine.py puzzle.json engine-check.json --katago /path/to/katago --model /path/to/model --config /path/to/analysis.cfg
```

Use the available primary model, not a second-opinion network. The helper searches the root and
all offered first choices, retaining raw responses, engine/model/config provenance and an exact
position/rules/komi fingerprint. With `method: "engine"`, set `engine_artifact` to the resulting
file’s path. The validator checks the match and rejects wrong choices that the searched scores
do not clearly separate from accepted answers. Inspect the PVs yourself for the teaching claim;
engine preferences do not prove unconditional life/death. Never label unsearched moves wrong.

When the teaching environment has no KataGo, use the bounded capture checker where applicable,
or externally verified solutions and careful reading with stated limits. Do not assume access
to the user’s laptop or request the application JSON. Run `validate_lesson.py` and inspect the
rendered lesson before delivering it. Keep these working files for debugging, but deliver the
usual single HTML lesson.
