---
name: go-game-teacher
description: "Transforms go_teacher/KataGo game review markdown into interactive HTML lessons for beginners. Use this skill whenever a user uploads a Go game review, analysis, or KataGo/KaTrain/go_teacher markdown and wants to learn from their mistakes. Triggers on phrases like 'analyse this Go game', 'review my Go game', 'help me improve at Go', or any mention of KataGo/KaTrain game reviews. Produces a self-contained HTML file with clickable board positions, graded explanations of the played move and its alternatives, practice puzzles that test the same concepts in fresh, non-obvious positions, and a phase-by-phase narrative of how the game developed (opening, middle game, endgame) that traces the cause-and-effect chains around each teaching moment, grounded in sequential web research with a focus on Go Magic tutorials."
allowed-tools: execute_code terminal read_file write_file workspace_output workspace_emit present_output web_search web_get_contents
---

# Go Game Teacher

Turn a go_teacher review markdown into an interactive HTML lesson that teaches a beginner what they got wrong, shows how good or bad each alternative was, and lets them practise the same idea on fresh puzzles. Alongside the tactics, it tells the story of how the game unfolded — how each teaching moment arose and what it led to — so the student sees the game as an arc, not a list of disconnected snapshots.

The markdown is the **only** input. The original SGF is not available, so every board fact you state must come from the report: the Teaching candidates section, the Game arc facts section, the diagrams, the candidate tables, the stone lists and the compact move list.

The report carries a `Report format: N` line. The parser refuses formats it does not understand; if that happens, tell the user the skill and go_teacher are out of step rather than guessing at the layout.

## What you need

- **Input**: one markdown file produced by go_teacher (KataGo analysis). See `references/katago-review-format.md`.
- **Output**: one self-contained HTML file with interactive Go boards, explanations and puzzles.

## Workflow

### 1. Parse the review (brief by default)

```
python <sandbox_dir>/scripts/parse_review.py <input.md> <parsed.json>
```

The default output is *brief*: game info, summary statistics, the **teaching candidates** with their board facts, refutation lines, chains and stone lists, the **arc facts** (per-phase numbers and life-and-death changes), praised moves, the compact move list, and scalar facts for every move. Candidate tables and diagrams are kept only for teaching candidates and praised moves. This is the file to read.

`game_info.two_pass` tells you whether key positions were re-analysed deeply; `move_details[n].deep` marks those moves. Deep numbers are the reliable ones: when a deep and a shallow figure disagree, quote the deep one.

Add `--full` only if you need the candidate table or diagram of a move outside the shortlist (for example an opponent blunder you want to mention). Do not read the whole markdown into context; if you need one move, search the markdown for `### Move N:`.

### 2. Identify the student

Read `game_info.student` (`"B"` or `"W"`) and `game_info.student_reason`. The program picks the side facing an engine-looking name (KaTrain writes bots as "AI (...)"), otherwise Black, unless the user overrode it. If the reason says "default" and the player names are empty or ambiguous, state in the overview which side you are treating as the student. Every lesson, praised move and statistic must refer to that side. Never assume Black.

### 3. Choose 2–3 teaching moments

Start from `teaching.candidates`. The program has already ranked the student's mistakes by point loss and dropped repeats from the same local fight; each entry carries facts you cannot compute yourself:

- `decided_before` and `winrate_before`: if the game was already decided, the winrate swing means nothing. Judge by `point_loss` only, and prefer candidates from the undecided part of the game when you have the choice. Beginner games on 9x9 flip between 1% and 99% every move; that is normal.
- `loss_region` and `loss_region_points`: where on the board the points changed hands.
- `better_move_is` ("same area" = shape or reading; "elsewhere" = direction or priority).
- `captured_at`: the played stone was captured later, a reading error.
- hints about atari, tenuki while threatened, and unnecessary local answers.
- `net_policy` / `human_policy`: how natural the move looked to KataGo's network and how often a player of the chosen human rank plays it. High human policy plus a large loss = a typical mistake for the level, worth a lesson. Very low human policy = an unusual slip.
- `theme`: the program's rule-based classification (reading / tactics, life and death, tenuki while threatened, over-defending / priority, shape / connection, endgame counting, direction of play). Use it as the default theme; override only with a stated reason.
- `refutation` and `refutation_score`: the opponent's strongest punishment after the played move (opponent moves first) and where it leads. This is *what the mistake allows* — the core of a "why it's wrong" explanation. `better_line` is KataGo's continuation after the preferred move.
- `status_changes`: groups whose life-and-death status this move changed (alive → dead, dead → alive, → captured), from the ownership maps. A move that kills your own group is a life-and-death lesson regardless of anything else.
- `chain_before` / `chain_after`: the moves played in the same area shortly before and after, each with its point loss and notes (captured at N, status changes). This is the raw material for the cause→effect story in step 3a.

Pick 2–3 candidates with distinct themes (see `references/go-teaching-concepts.md`); prefer candidates with a non-empty `refutation` and a clear `theme`. Fall back to `summary.biggest_mistakes` filtered by `player` only if fewer than two candidates exist.

### 3a. Trace the arc of the game

Before writing anything, follow `references/game-arc-commentary.md` to build the game's story. In short:

- **Segment the game into phases** — the report does this for you: `arc.phases` gives each phase's move range, Black's winrate and the score at both ends, mean loss per side, the worst move per side, the student's top-1 rate, and the *hot regions* where ownership changed most. `arc.status_changes` lists every group that was killed, saved or captured, by move. Write 2–4 sentences per phase from these numbers: what each side was trying to do, the one or two moves that mattered most, and how the balance shifted.
- **Trace a cause→effect chain around every shortlisted move** — start from the candidate's `chain_before` and `chain_after` (moves in the same area with their losses and notes), its `status_changes`, `captured_at` and `refutation`. Backward: which earlier move in the chain set up the problem (the mistake is rarely the first error in its chain). Forward: what the refutation shows the move allowing, and what actually happened in the chain ("the atari at 23 was answered too late; by 27 the group had to sacrifice two stones, and that sacrifice made White's centre thick — which is why the 31 invasion failed"). Only the moves listed in the chains and the status table are safe to cite as connected; do not invent links between distant moves.
- **Map each phase and chain onto a named pattern** — playing under strong stones, premature invasion, urgent-before-big, ladder direction — and carry those names into step 4's searches.

Every arc claim obeys Board-fact discipline (below): only report captures, atari and liberties the report supports.

### 4. Research patterns, remedies and classic examples online

Run the research as a **sequence** of searches that follows how the game developed, not one search per mistake — the full methodology is in `references/game-arc-commentary.md` (phase searches, chain searches, pattern-name searches, pro-example searches). Then, for each chosen mistake, read the promising results in full. Look for:

- **The pattern name** ("playing under strong stones", "empty triangle", "premature invasion", "tenuki when threatened", "ignoring atari", "reverse sente"). Search the theme + "Go" + "beginner" or "mistake".
- **How teachers explain it**: Sensei's Library, Go Magic, learn-go.net, GoProblems, books, videos. You want memorable phrasing, analogies and the standard corrective advice.
- **How to avoid it**: drills, habits and rules of thumb.
- **Classic example positions** of the same idea (a well-known tsumego, a proverb illustration, a joseki deviation). These seed the puzzles in step 6. Note the shape, not the coordinates.

Also search for the specific shape if it maps to a known joseki or fuseki pattern. Distill 2–3 bullets per mistake, and one cause→effect chain per lesson. Research is enrichment, not a blocker, but do it thoroughly; it is what makes the lesson better than the raw numbers.

**Go Magic first.** Check the verified Go Magic tutorial library in `references/game-arc-commentary.md` for a course or free lesson covering the concept, and make it the first resource link. Go Magic's video lessons are unusually beginner-friendly; the free lesson "Three Stages of the Game" (https://gomagic.org/lessons/three-stages-of-the-game/) is the model for framing phase transitions. Re-verify any slug you cite that is not in the library: search gomagic.org for the exact course title before linking.

### 5. Write the lesson content

For each chosen mistake write:

- **What happened**: the position, the opponent's last move, what the student played, what KataGo preferred. Use the hints and `loss_region` to say *where* and *why* in concrete terms.
- **Why it's wrong**: the strategic or tactical concept, in beginner language, grounded in your research. Name the pattern if it has a name. Lead with what the move *allows*: walk the `refutation` (the opponent's strongest reply and continuation) in 2–3 sentences and put it in `refutation_explanation`. The HTML has a "What it allows" button that plays this sequence on the board with numbered stones, so refer to the numbers ("after 1 and 3 the D3 stones have one liberty").
- **The variation**: walk through KataGo's `better_line` (the same as `preferred_pv`) and explain the significant moves in `variation_explanation`; copy the line into `better_line` so the "Better line" button can play it.
- **Life and death**: when `status_changes` says a group died or was captured because of this move, say so in plain words with the group's anchor point and stone count, and make the lesson about the status of that group.
- **Graded alternatives**: from the move's candidate table (`candidates`, with `loss_vs_best`), take 2–4 other moves a beginner might consider and write one or two sentences each on *why* it is worse (or nearly as good). Set `loss_vs_best` and `quality` (best / good / inaccuracy / mistake / big mistake / blunder, thresholds 0.5 / 1.5 / 3 / 6 / 12 points). Include the played move's own quality as `played_quality`. The HTML shows each alternative on the board with a colour for its quality and a "Show all candidates" overlay.
- **Level framing**: when human-policy numbers exist, say whether this is a common move at the student's level and what a stronger player would do instead ("a 5k plays this 30% of the time; a 1d almost always plays D4 because ...").
- **The principle**: one sentence the student can remember, phrased as a habit.

Also write the arc and summary content:

- **The overall game summary** (`overall_feedback`, 2–3 paragraphs): what the student does well, the main weakness, which phase needs work. Use `summary.accuracy`, `arc.phases` (mean loss and top-1 rate per phase) and the theme counts across candidates, and say which side the student is.
- **The game arc** (`game_arc`): a short `intro` plus one entry per phase (opening / middle game / endgame) with `phase`, `move_range`, a `narrative` of 2–4 sentences, and optional `turning_points` (short move-tagged notes). This is the holistic commentary — how the strategic balance swung, not a move-by-move list. Follow `references/game-arc-commentary.md` on voice and board-fact discipline.
- **The lesson story** (`story` on each lesson): 2–4 sentences tracing the cause→effect chain around that move — how the position arose and what the mistake led to, anchored in move numbers taken from `chain_before`, `chain_after` and `status_changes`. Keep it distinct from `explanation` (the tactical why) — the story is the context.
- **Theme**: copy the candidate's `theme` into the lesson's `theme` field (shown as a badge) and choose `concept` / `concept_label` to match it.

### 5a. Highlight moves played well

Use `teaching.praise` first: moves where the student found KataGo's first choice and the second-best was clearly worse in a live position. Add 1–2 more from the compact move list if needed (rank #1, low loss, not a forced or trivial move). Give each a fun, generous kyu/dan rating and a beginner-friendly explanation.

### 6. Design practice puzzles

For each lesson concept, design 1–2 puzzles. Rules:

- **Not from the game.** Never reuse the game position or a trivially edited copy of it.
- **Non-obvious variations.** Start from the classic examples you found in step 4, then transform them: rotate or mirror, swap colours, move the fight to a different side or corner, add a stone or two that changes the reading without changing the lesson. The right answer should require applying the principle, not pattern-matching the lesson board.
- **One concept, one clear best move.** Verify the position is coherent: no overlapping stones, no stones off the board, the correct move is on an empty point, groups you call "in atari" really have one liberty, the opponent's last move is a stone that is on the board.
- **Graded wrong moves with punishments.** List 2–3 tempting wrong moves in `wrong_moves`, each with `quality`, an estimated `loss_vs_best`, an explanation of *why it falls short*, and a `refutation`: the opponent's 1–4 reply moves that punish it, as GTP coordinates on the puzzle board. When the student clicks a wrong move the HTML plays that punishment with numbered stones, which is far more convincing than a sentence. Model the punishments on the lesson's `refutation`: the puzzle should fail for the same *reason* the game move failed, in a different shape. Also give `generic_wrong_explanation` for clicks that hit none of them. Verify every refutation move lands on an empty point of the puzzle position after the wrong move.
- **Life-and-death puzzles.** When a lesson's theme is life and death, build the puzzle around a group that lives or dies with one move, and use the `status_changes` vocabulary ("this group has one eye; find the move that makes the second").
- Include `opponent_last_move` so the student has context, a `hint`, and an `explanation` of the correct move.

Keep positions simple enough to read out mentally: 9x9 or a corner/side fragment of a larger board.

### 6a. Write the Go concepts & resources section

One `concepts_learned` entry per concept: English term, Japanese term with kanji and romaji, a 2–3 sentence description, 2–3 resource links found in your research, and a memorable anecdote or proverb. `references/go-teaching-concepts.md` has terms and anecdotes to draw on.

### 7. Generate the HTML

Write the lesson JSON following `references/html-build-guide.md`. Before generating, check that `game_arc` is present with at least one phase and that every lesson has a non-empty `story` — the generator will silently skip missing arc content, and the lesson is incomplete without it. Then:

```
python <sandbox_dir>/scripts/generate_lesson.py <parsed.json> <lesson.json> <output.html>
```

### 8. Deliver

Save the HTML and deliver it. If writing with Python (which may seek), write to `/scratch/work/` first, then promote with `workspace_emit`, then `present_output`.

## Board-fact discipline

The student will check your claims against the board. Only state what the report supports:

- Captures: only if `captured_at` or a `status_changes` row says so, or the diagram / compact move list shows it.
- Life-and-death status (alive, dead, unsettled): only from `status_changes` / `arc.status_changes`.
- Punishing sequences: quote `refutation` for the game position; for puzzles you design the sequence yourself and must verify it on the puzzle board.
- Atari and liberties: only from the hints ("in atari: ...") or by counting on a diagram you can see.
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

- `references/katago-review-format.md` — the markdown structure (format 3), every field including refutations, chains, themes and arc facts, how to read diagrams and tables
- `references/go-teaching-concepts.md` — mistake themes, decided games, human-policy framing, grading alternatives, puzzle variation design
- `references/game-arc-commentary.md` — phase segmentation, cause→effect chain tracing, sequential search methodology, arc narrative voice, verified Go Magic tutorial library
- `references/html-build-guide.md` — lesson JSON schema (game arc, story, alternatives, wrong moves), HTML behaviour
