---
name: go-game-teacher
description: "Transforms go_teacher/KataGo game review markdown into interactive HTML lessons for beginners. Use this skill whenever a user uploads a Go game review, analysis, or KataGo/KaTrain/go_teacher markdown and wants to learn from their mistakes. Triggers on phrases like 'analyse this Go game', 'review my Go game', 'help me improve at Go', or any mention of KataGo/KaTrain game reviews. Produces a self-contained HTML file with clickable board positions, graded explanations of the played move and its alternatives, practice puzzles that test the same concepts in fresh, non-obvious positions, and a phase-by-phase narrative of how the game developed (opening, middle game, endgame) that traces the cause-and-effect chains around each teaching moment, grounded in sequential web research with a focus on Go Magic tutorials."
allowed-tools: execute_code terminal read_file write_file workspace_output workspace_emit present_output web_search web_get_contents
---

# Go Game Teacher

Turn a go_teacher review markdown into an interactive HTML lesson that teaches a beginner what they got wrong, shows how good or bad each alternative was, and lets them practise the same idea on fresh puzzles. Alongside the tactics, it tells the story of how the game unfolded — how each teaching moment arose and what it led to — so the student sees the game as an arc, not a list of disconnected snapshots.

The main purpose is to isolate mistakes and teach better choices through replies, alternatives and practice. Keep the existing lesson sections, interactive panels, praise and puzzles. The game overview supplies context: choose a few supported strategic observations, then give most of the attention to the teaching moments. Do not turn the full move list into a move-by-move narration or repeat the same takeaway in the overview, each phase and each lesson.

The main `.md` report is the **only game input**. Do not ask for the app's separate `.json` or the original SGF. The `parsed.json` and `brief.json` used below are created by this skill from the Markdown, not additional user uploads. Every board fact must come from the report: format 5's structured evidence block, or the teaching candidates, arc facts, diagrams, candidate tables, stone lists and move list in older reports.

The report carries a `Report format: N` line. The parser refuses formats it does not understand; if that happens, tell the user the skill and go_teacher are out of step rather than guessing at the layout.

## What you need

- **Input**: one markdown file produced by go_teacher (KataGo analysis). See `references/katago-review-format.md`.
- **Output**: one self-contained HTML file with interactive Go boards, explanations and puzzles.

## Workflow

### 1. Parse the review

```
python <sandbox_dir>/scripts/parse_review.py <input.md> <parsed.json> --brief-output <brief.json> --facts-output <facts.json>
```

**Format 5:** also read [references/grounding-and-practice.md](references/grounding-and-practice.md) for the derived fact sheet, draft claim checks and verified transfer puzzles. New lessons set `grounding_version: 1`. First read [references/evidence-format5.md](references/evidence-format5.md). It defines the new evidence contract, which computations to use in lessons and practice, their limits, and lesson schema 2. Read brief.json to choose moments and inspect the compact game timeline and summaries, then the complete selected evidence in parsed.json to explain exact lines. New format-5 reports add point loss and score to every move; older format-5 reports may lack those optional fields. Always generate from parsed.json. Write prose and move references; the generator loads numbers, boards and sequences directly. The report includes setup stones and White-to-play positions. Large heatmap arrays are not in the Markdown.

**Formats 2–4:** the following legacy parsing details apply.

The default output is *brief*: game info, summary statistics, the **teaching candidates** with their board facts, refutation lines, chains and stone lists, the **arc facts** (per-phase numbers and life-and-death changes), praised moves, the compact move list, and scalar facts for every move. Candidate tables and diagrams are kept only for teaching candidates and praised moves. This is the file to read.

`game_info.two_pass` tells you whether key positions were re-analysed deeply; `move_details[n].deep` marks those moves. Prefer the deeper search for the same position, but keep its provenance and uncertainty; additional visits are not a correctness guarantee.

Format 4 adds four optional blocks the parser exposes when present: `openings` (how each corner was played, with KataGo's first deviation), `history` (this game against the student's earlier games), `time_and_loss` plus per-move `time_spent_seconds` (when the record has clocks), and the target-profile human policy (`target_policy` on candidates, `target_policy_*` on moves).

For formats 2–4, add `--full` to `parse_review.py` only if you need the candidate table or diagram of a move outside the shortlist (for example an opponent blunder you want to mention). Do not read the whole Markdown into context; search for `### Move N:`. Format 5 always parses the complete evidence block; use its optional brief reading view to choose moments.

### 1a. Validate before generating

```
python <sandbox_dir>/scripts/validate_lesson.py <parsed.json> <lesson.json>
```

Run this on every lesson JSON before step 7. It checks that lesson moves exist and were played by the stated colour, that every coordinate is legal and on an empty point (alternatives, refutations, better lines, puzzle stones, correct and wrong moves, punishment sequences), that no puzzle reuses a game position, and that every wrong move has an explanation. For grounded drafts it also checks explicit fact assertions, replayed puzzle liberties, and bounded capture objectives against all legal replies. Fix every PROBLEM line; read the warnings. It does not certify the meaning of unrestricted prose, puzzle difficulty or unconditional life/death: compare the actual sentences to facts.json before rendering.

### 2. Identify the student

Read `game_info.student` (`"B"` or `"W"`) and `game_info.student_reason`. The program picks the side facing an engine-looking name (KaTrain writes bots as "AI (...)"), otherwise Black, unless the user overrode it. If the reason says "default" and the player names are empty or ambiguous, state in the overview which side you are treating as the student. Every lesson, praised move and statistic must refer to that side. Never assume Black.

### 3. Choose 2–3 teaching moments

Start from `teaching.candidates`. The program has already ranked the student's mistakes by point loss and dropped repeats from the same local fight; each entry carries facts you cannot compute yourself:

- `engine_strongly_favoured_side_before` (legacy `decided_before`) and `winrate_before`: engine winrate saturation can hide large mistakes. Prefer point loss, and retain later lead reversals in the game arc. Do not say a human game was decided from a single saturated evaluation.
- `loss_region` and `loss_region_points`: where on the board the points changed hands.
- `better_move_is` ("same area" = shape or reading; "elsewhere" = direction or priority).
- `captured_at`: the played stone was captured later, a reading error.
- hints about atari, tenuki while threatened, and unnecessary local answers.
- `net_policy` / `human_policy`: how natural the move looked to KataGo's network and the probability assigned by the chosen human profile. High human policy plus a large loss = a typical mistake for the level, worth a lesson. Very low human policy = an unusual slip.
- `theme`: the program's rule-based classification (reading / tactics, life and death, tenuki while threatened, over-defending / priority, shape / connection, endgame counting, direction of play). Use it as the default theme; override only with a stated reason.
- `refutation` and `refutation_score`: the opponent's strongest punishment after the played move (opponent moves first) and where it leads. This is *what the mistake allows* — the core of a "why it's wrong" explanation. `better_line` is KataGo's continuation after the preferred move.
- `status_changes`: predicted group ownership changes, plus actual captures. Legacy alive/dead labels are predictions under subsequent play, not proofs. Use restricted searches and evaluated lines for stronger tactical explanations.
- `chain_before` / `chain_after`: the moves played in the same area shortly before and after, each with its point loss and notes (captured at N, status changes). This is the raw material for the cause→effect story in step 3a.
- `target_policy` (when a stronger target profile was chosen): the probability assigned by the stronger target profile to the played move and its preferred choice. Together with `human_policy` it gives the level framing directly: "the 5k model assigns D4 60% probability here".
- `time_spent_seconds` and `fast` (when the record has clocks): timing can motivate a count-first habit, but does not establish why the mistake happened.

Pick 2–3 candidates with distinct themes (see `references/go-teaching-concepts.md`); prefer candidates with a non-empty `refutation` and a clear `theme`. Fall back to `summary.biggest_mistakes` filtered by `player` only if fewer than two candidates exist.

### 3a. Trace the arc of the game

Use `references/game-arc-commentary.md` to connect the selected lessons to the game's development. The arc is supporting context; do not manufacture a theme for every phase. For format 5, follow the compact-context guidance in `references/evidence-format5.md`. In short:

- **Segment the game into phases** — the report does this for you: `arc.phases` gives each phase's move range, Black's winrate and the score at both ends, mean loss per side, the worst move per side, the student's top-1 rate, and the *hot regions* where ownership changed most. `arc.status_changes` lists predicted ownership changes and observed captures, by move. Write a short account of each phase using those numbers: how the balance shifted and the useful decisions behind it when supported by the teaching evidence. One or two sentences can be enough; expand only when it clarifies a lesson.
- **Trace a cause→effect chain around every shortlisted move** — start from the candidate's `chain_before` and `chain_after` (moves in the same area with their losses and notes), its `status_changes`, `captured_at` and `refutation`. Backward: which earlier move in the chain set up the problem (the mistake is rarely the first error in its chain). Forward: what the refutation shows the move allowing, and what actually happened in the chain ("the atari at 23 was answered too late; by 27 the group had to sacrifice two stones, and that sacrifice made White's centre thick — which is why the 31 invasion failed"). Only the moves listed in the chains and the status table are safe to cite as connected; do not invent links between distant moves.
- **Research the patterns supported by the selected phases and chains** — playing under strong stones, premature invasion, urgent-before-big, ladder direction — and carry those names into step 4's searches. Do not force a named pattern onto a phase that does not support one.
- **Use supporting positions when needed.** In new format-5 reports, `context_moves` identifies the opponent's biggest mistakes, checkpoints and last move; `move_details[str(number)]` has their evaluations and searched alternatives. Read those entries from parsed.json to support a transition or an opportunity the student missed. Teaching candidates and praised moves retain their existing evidence. Do not make every supporting position a separate lesson.
- **Use `openings` for the opening phase.** Each corner comes with a summary such as "4-4 point, small knight's approach, 3-3 invasion" and the first move KataGo disliked there. Research that exact pattern (step 4) and let the opening phase narrative say which corner went wrong and how. Geometric corner labels and first deviations are research leads, not validated joseki recognition. Verify the pattern before naming a joseki.
- **Use current statistics for this game.** In format 5, read `facts.current` and use only `facts.history.prior_games` for comparisons. Possible earlier analyses of the same game are separated and excluded. The renderer derives `progress.rows` from these facts; write its supported summary. Legacy reports may supply an existing history table, but never substitute an older row for current statistics or invent a trend.
- **Use `summary.timing` in new format-5 reports, or legacy `time_and_loss` when present**: compare recorded times for the student, checking sample counts and missing data. A timing/loss association can suggest a count-first habit; it cannot prove carelessness or time pressure. In format 5 each player has their own median split; do not compare the student with the bot's speed.

Every arc claim obeys Board-fact discipline (below): only report captures, atari and liberties the report supports.

### 4. Research patterns, remedies and classic examples online

Run the research as a **sequence** of searches that follows how the game developed, not one search per mistake — the full methodology is in `references/game-arc-commentary.md` (phase searches, chain searches, pattern-name searches, pro-example searches). Then, for each chosen mistake, read the promising results in full. Look for:

- **The pattern name** ("playing under strong stones", "empty triangle", "premature invasion", "tenuki when threatened", "ignoring atari", "reverse sente"). Search the theme + "Go" + "beginner" or "mistake".
- **How teachers explain it**: Sensei's Library, Go Magic, learn-go.net, GoProblems, books, videos. You want memorable phrasing, analogies and the standard corrective advice.
- **How to avoid it**: drills, habits and rules of thumb.
- **Classic example positions** of the same idea (a well-known tsumego, a proverb illustration, a joseki deviation). These seed the puzzles in step 6. Note the shape, not the coordinates. When you later need a classic problem's exact stones, take them from the machine-readable collections described in [references/tsumego-source-formats.md](references/tsumego-source-formats.md) — never transcribe from image diagrams.

Also search for the specific shape if it maps to a known joseki or fuseki pattern. Distill 2–3 bullets per mistake, and one cause→effect chain per lesson. Research thoroughly with no preset search, source or time cap. Continue until explanations and external puzzle examples are supported; if access fails, state what could not be verified.

**Go Magic first.** Check the verified Go Magic tutorial library in `references/game-arc-commentary.md` for a course or free lesson covering the concept, and make it the first resource link. Go Magic's video lessons are unusually beginner-friendly; the free lesson "Three Stages of the Game" (https://gomagic.org/lessons/three-stages-of-the-game/) is the model for framing phase transitions. Re-verify any slug you cite that is not in the library: search gomagic.org for the exact course title before linking.

### 5. Write the lesson content

For each chosen mistake write:

- **What happened**: the position, the opponent's last move, what the student played, what KataGo preferred. Use the hints and `loss_region` to say *where* and *why* in concrete terms.
- **Why it's wrong**: the strategic or tactical concept, in beginner language, grounded in your research. Name the pattern if it has a name. Lead with what the move *allows*: walk the `refutation` (the opponent's strongest reply and continuation) in 2–3 sentences and put it in `refutation_explanation`. The Your move panel plays this sequence with numbered stones, so refer to the numbers ("after 1 and 3 the D3 stones have one liberty").
- **The variation**: walk through KataGo's `better_line` (the same as `preferred_pv`) and explain the significant moves in `variation_explanation`; for schema 2 reference the evidence and write the Best panel text; for legacy schema 1 copy the line into `better_line`.
- **Life and death**: when `status_changes` predicts a group ownership loss, state it as a prediction; when the actual board records a capture, state that capture with the group's anchor point and stone count, and make the lesson about the status of that group.
- **Graded alternatives**: from the move's candidate table (`candidates`, with `loss_vs_best`), take 2–4 other moves a beginner might consider and write one or two sentences each on *why* it is worse (or nearly as good). Set `loss_vs_best` and `quality` (best / good / inaccuracy / mistake / big mistake / blunder, thresholds 0.5 / 1.5 / 3 / 6 / 12 points). Include the played move's own quality as `played_quality`. The HTML shows each alternative on the board with a colour for its quality and a "Show all candidates" overlay.
- **Level framing**: when human-policy numbers exist, say whether this is a common move at the student's level and what a stronger player would do instead, and put it in the lesson's `level_framing` field (rendered as an "At your level" callout). Prefer the target-profile numbers when they exist: "the 10k profile assigns the played move 30%; the 3k profile assigns it much less and prefers D4 (60%)". Bind each value to its actual move, check the comparison direction, and describe model probabilities rather than observed frequencies.
- **The principle**: one sentence the student can remember, phrased as a habit.

Also write the arc and summary content:

- **The overall game summary** (`overall_feedback`, 2–3 short paragraphs): what the student does well, the main weakness, which phase needs work. Use `summary.accuracy`, `arc.phases` (mean loss and top-1 rate per phase) and the theme counts across candidates, and say which side the student is.
- **The game arc** (`game_arc`): a short `intro` plus one entry per phase (opening / middle game / endgame) with `phase`, `move_range`, a brief `narrative` (expand only when useful), and optional `turning_points` (short move-tagged notes). This is the holistic commentary — how the strategic balance swung, not a move-by-move list. The Game Overview and arc prose sit beside a persistent context board: every `move N` and every uniquely-played coordinate you mention becomes a clickable jump to that position, so reference moves naturally by number or point. Each phase may also set an optional integer `anchor_move` (and the root an `overview_anchor_move`) choosing the position its card shows — default is the end of the phase's move range / the final position. Follow `references/game-arc-commentary.md` on voice and board-fact discipline.
- **The lesson story** (`story` on each lesson): 2–4 sentences tracing the cause→effect chain around that move — how the position arose and what the mistake led to, anchored in move numbers taken from `chain_before`, `chain_after` and `status_changes`. Keep it distinct from `explanation` (the tactical why) — the story is the context.
- **Theme**: copy the candidate's `theme` into the lesson's `theme` field (shown as a badge) and choose `concept` / `concept_label` to match it.
- **Progress** (`progress`, optional): when `history` exists, a `summary` paragraph on the trend and `rows` copied from `history.metrics` (`metric`, `this_game`, `recent_average`). Rendered as a "Your Progress" card after the game arc.

### 5a. Highlight moves played well

Use `teaching.praise` first: moves where the student found KataGo's first choice and the second-best was clearly worse in a live position. Add 1–2 more only if the report supplies supporting rank/alternative evidence (rank #1, low loss, not a forced or trivial move). The new format-5 timeline supplies loss and score, not per-move ranks; low loss alone is insufficient for an "only good move" claim. Give each a beginner-friendly explanation of what the student did well. Avoid invented kyu/dan ratings.

### 6. Design practice puzzles

For each lesson concept, design 1–2 puzzles. Rules:

- **Not from the game.** Never reuse the game position or a trivially edited copy of it.
- **Non-obvious variations.** Start from the classic examples you found in step 4, then transform them: rotate or mirror, swap colours, move the fight to a different side or corner, add a stone or two that changes the reading without changing the lesson. The right answer should require applying the principle, not pattern-matching the lesson board. When a classic tsumego seeds the puzzle, get its exact position with `scripts/parse_tasuki_tex.py` and verify its bounded objective with `scripts/solve_tsumego.py` (crop + mechanical search) as described in [references/tsumego-source-formats.md](references/tsumego-source-formats.md), rather than reconstructing it from diagrams. If a search feels slow, follow that reference's fallback ladder — never substitute visual design or "quickly verifiable" hand-made positions for mechanical verification.
- **One concept, one explicit objective.** Follow grounding-and-practice.md; distinguish bounded capture/survival from a searched preference. Accept all equally valid verified answers. Do not claim unique whole-board optimality from a sparse puzzle or a finite search. Verify the position is coherent: no overlapping stones, no stones off the board, the correct move is on an empty point, groups you call "in atari" really have one liberty, the opponent's last move is a stone that is on the board.
- **Graded wrong moves with punishments.** List 2–3 tempting wrong moves in `wrong_moves`, each with `quality`, an explanation of *why it falls short*, and a `refutation`: the alternating continuation, beginning with the opponent's reply, through the strongest defence and necessary follow-up, as GTP coordinates on the puzzle board. When the student clicks a wrong move the HTML plays that punishment with numbered stones, which is far more convincing than a sentence. Model the punishments on the lesson's `refutation`: the puzzle should fail for the same *reason* the game move failed, in a different shape. Also give `generic_wrong_explanation` for clicks that hit none of them. Replay the complete line with captures, passes and ko; a captured point can legally become available again. For schema 2 include source, transformation, transfer relation, verification and correct_lines as specified in evidence-format5.md.
- **Life-and-death puzzles.** When a lesson's theme is life and death, build the puzzle around a group that lives or dies with one move, and use the `status_changes` vocabulary ("this group has one eye; find the move that makes the second").
- **Opening puzzles.** When the lesson comes from an `openings` first deviation, the puzzle is a corner position from the same pattern family (found in research) transformed to another corner or colour, asking for the standard move; wrong moves are the deviation and one other tempting mistake, with their punishments.
- **Count-first puzzles.** When a selected lesson involves a reading mistake and recorded timing suggests a useful count-first habit, consider a capturing race or atari sequence where the obvious quick move loses and the answer needs one more liberty counted. A timing statistic alone does not require an extra puzzle or establish time pressure.
- **Ground and verify.** Include the specific external example and original board, meaningful transformation, objective, strongest-defence reading, and replayed board assertions from grounding-and-practice.md. Use its optional KataGo checker when available. The original source solution does not automatically verify a transformed board.
- **Validate.** Run `scripts/validate_lesson.py` (step 1a) and fix every problem before generating.
- Include `opponent_last_move` so the student has context, a `hint`, and an `explanation` of the correct move.

Keep positions simple enough to read out mentally: 9x9 or a corner/side fragment of a larger board.

### 6a. Write the Go concepts & resources section

One `concepts_learned` entry per concept: English term, Japanese term with kanji and romaji, a 2–3 sentence description, 2–3 resource links found in your research, and a memorable anecdote or proverb. `references/go-teaching-concepts.md` has terms and anecdotes to draw on.

### 7. Generate the HTML

For format 5 write schema 2 from `references/evidence-format5.md`; use `references/html-build-guide.md` for the retained narrative/resource fields and legacy schema. Run the validator (step 1a); it also checks that `game_arc` has phases and every lesson has a `story`. Then:

```
python <sandbox_dir>/scripts/generate_lesson.py <parsed.json> <lesson.json> <output.html>
```

### 8. Deliver

Save the HTML and deliver it. If writing with Python (which may seek), write to `/scratch/work/` first, then promote with `workspace_emit`, then `present_output`.

## Board-fact discipline

The student will check your claims against the board. Only state what the report supports:

- Captures: only if `captured_at` or a `status_changes` row says so, or the diagram / compact move list shows it.
- Life-and-death: legacy status_changes labels are ownership predictions. Only state unconditional life/death, seki or eye counts when the reading or verified board facts establish them.
- Punishing sequences: quote `refutation` for the game position; for puzzles you design the sequence yourself and must verify it on the puzzle board.
- Atari and liberties: verify using `go_rules.Position.group` and `board_checks` for the exact replayed position; do not infer safety from a group gaining liberties.
- "Where the points went": use `loss_region`.
- Do not invent ladders, nets or ko fights that the PV does not show. If you want to demonstrate a tactic beyond the PV, say that it is your own illustration.
- Coordinates: GTP letters A–T without I, rows from the bottom. Check every coordinate you write against the stone lists.

## Tone and style

Write as a patient teacher talking to a beginner:

- Explain WHY, not just WHAT. "E7 connects your stones so White cannot cut" beats "E7 is 7 points better."
- Concrete language: "this stone is in atari", not "suboptimal tactical properties".
- Encouraging: mistakes are learning opportunities. Praise the good moves genuinely.
- Connect each lesson to a memorable habit.

## Reference files

- `references/tsumego-source-formats.md` — machine-readable tsumego sources (tasuki tex corpus, pregenerated SGFs, solution trees): formats, decoding rules, coordinate conventions, the verified pitfalls, and the parser/solver scripts (`parse_tasuki_tex.py`, `solve_tsumego.py`)
- `references/grounding-and-practice.md` — deterministic facts, checked draft claims, externally sourced variations and strongest-defence verification

- `references/evidence-format5.md` — format 5, schema 2, evidence-driven panels and researched practice variations
- `references/katago-review-format.md` — legacy markdown structure (formats 2–4), every field including refutations, chains, themes and arc facts, how to read diagrams and tables
- `references/go-teaching-concepts.md` — mistake themes, decided games, human-policy framing, grading alternatives, puzzle variation design
- `references/game-arc-commentary.md` — phase segmentation, cause→effect chain tracing, sequential search methodology, arc narrative voice, verified Go Magic tutorial library
- `references/html-build-guide.md` — lesson JSON schema (game arc, story, alternatives, wrong moves), HTML behaviour
