---
name: go-game-teacher
description: "Transforms go_teacher/KataGo game review markdown into interactive HTML lessons for beginners. Use this skill whenever a user uploads a Go game review, analysis, or KataGo/KaTrain/go_teacher markdown and wants to learn from their mistakes. Triggers on phrases like 'analyse this Go game', 'review my Go game', 'help me improve at Go', or any mention of KataGo/KaTrain game reviews. Produces a self-contained HTML file with clickable board positions, graded explanations of the played move and its alternatives, and practice puzzles that test the same concepts in fresh, non-obvious positions."
allowed-tools: execute_code terminal read_file write_file workspace_output workspace_emit present_output web_search web_get_contents
---

# Go Game Teacher

Turn a go_teacher review markdown into an interactive HTML lesson that teaches a beginner what they got wrong, shows how good or bad each alternative was, and lets them practise the same idea on fresh puzzles.

The markdown is the **only** input. The original SGF is not available, so every board fact you state must come from the report: the Teaching candidates section, the diagrams, the candidate tables, the stone lists and the compact move list.

## What you need

- **Input**: one markdown file produced by go_teacher (KataGo analysis). See `references/katago-review-format.md`.
- **Output**: one self-contained HTML file with interactive Go boards, explanations and puzzles.

## Workflow

### 1. Parse the review (brief by default)

```
python <sandbox_dir>/scripts/parse_review.py <input.md> <parsed.json>
```

The default output is *brief*: game info, summary statistics, the **teaching candidates** with their board facts and stone lists, praised moves, the compact move list, and scalar facts for every move. Candidate tables and diagrams are kept only for teaching candidates and praised moves. This is the file to read.

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

Pick 2–3 candidates with distinct themes (see `references/go-teaching-concepts.md`). Fall back to `summary.biggest_mistakes` filtered by `player` only if fewer than two candidates exist.

### 4. Research patterns, remedies and classic examples online

For each chosen mistake, search the web and read the promising results in full. Look for:

- **The pattern name** ("playing under strong stones", "empty triangle", "premature invasion", "tenuki when threatened", "ignoring atari", "reverse sente"). Search the theme + "Go" + "beginner" or "mistake".
- **How teachers explain it**: Sensei's Library, Go Magic, learn-go.net, GoProblems, books, videos. You want memorable phrasing, analogies and the standard corrective advice.
- **How to avoid it**: drills, habits and rules of thumb.
- **Classic example positions** of the same idea (a well-known tsumego, a proverb illustration, a joseki deviation). These seed the puzzles in step 6. Note the shape, not the coordinates.

Also search for the specific shape if it maps to a known joseki or fuseki pattern. Distill 2–3 bullets per mistake. Research is enrichment, not a blocker, but do it thoroughly; it is what makes the lesson better than the raw numbers.

### 5. Write the lesson content

For each chosen mistake write:

- **What happened**: the position, the opponent's last move, what the student played, what KataGo preferred. Use the hints and `loss_region` to say *where* and *why* in concrete terms.
- **Why it's wrong**: the strategic or tactical concept, in beginner language, grounded in your research. Name the pattern if it has a name.
- **The variation**: walk through KataGo's preferred PV (`preferred_pv`) and explain the significant moves.
- **Graded alternatives**: from the move's candidate table (`candidates`, with `loss_vs_best`), take 2–4 other moves a beginner might consider and write one or two sentences each on *why* it is worse (or nearly as good). Set `loss_vs_best` and `quality` (best / good / inaccuracy / mistake / big mistake / blunder, thresholds 0.5 / 1.5 / 3 / 6 / 12 points). Include the played move's own quality as `played_quality`. The HTML shows each alternative on the board with a colour for its quality and a "Show all candidates" overlay.
- **Level framing**: when human-policy numbers exist, say whether this is a common move at the student's level and what a stronger player would do instead ("a 5k plays this 30% of the time; a 1d almost always plays D4 because ...").
- **The principle**: one sentence the student can remember, phrased as a habit.

Also write the overall game summary (2–3 paragraphs): what the student does well, the main weakness, which phase needs work. Use `summary.accuracy` and the phase table, and say which side the student is.

### 5a. Highlight moves played well

Use `teaching.praise` first: moves where the student found KataGo's first choice and the second-best was clearly worse in a live position. Add 1–2 more from the compact move list if needed (rank #1, low loss, not a forced or trivial move). Give each a fun, generous kyu/dan rating and a beginner-friendly explanation.

### 6. Design practice puzzles

For each lesson concept, design 1–2 puzzles. Rules:

- **Not from the game.** Never reuse the game position or a trivially edited copy of it.
- **Non-obvious variations.** Start from the classic examples you found in step 4, then transform them: rotate or mirror, swap colours, move the fight to a different side or corner, add a stone or two that changes the reading without changing the lesson. The right answer should require applying the principle, not pattern-matching the lesson board.
- **One concept, one clear best move.** Verify the position is coherent: no overlapping stones, no stones off the board, the correct move is on an empty point, groups you call "in atari" really have one liberty, the opponent's last move is a stone that is on the board.
- **Graded wrong moves.** List 2–3 tempting wrong moves in `wrong_moves`, each with `quality`, an estimated `loss_vs_best`, and an explanation of *why it falls short* (what the opponent does next, what it fails to achieve). Also give `generic_wrong_explanation` for clicks that hit none of them. The HTML shows the graded feedback per wrong move and a "Show evaluations" overlay comparing all listed moves.
- Include `opponent_last_move` so the student has context, a `hint`, and an `explanation` of the correct move.

Keep positions simple enough to read out mentally: 9x9 or a corner/side fragment of a larger board.

### 6a. Write the Go concepts & resources section

One `concepts_learned` entry per concept: English term, Japanese term with kanji and romaji, a 2–3 sentence description, 2–3 resource links found in your research, and a memorable anecdote or proverb. `references/go-teaching-concepts.md` has terms and anecdotes to draw on.

### 7. Generate the HTML

Write the lesson JSON following `references/html-build-guide.md`, then:

```
python <sandbox_dir>/scripts/generate_lesson.py <parsed.json> <lesson.json> <output.html>
```

### 8. Deliver

Save the HTML and deliver it. If writing with Python (which may seek), write to `/scratch/work/` first, then promote with `workspace_emit`, then `present_output`.

## Board-fact discipline

The student will check your claims against the board. Only state what the report supports:

- Captures: only if `captured_at` says so or the diagram / compact move list shows it.
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

- `references/katago-review-format.md` — the markdown structure, every field, how to read diagrams and tables
- `references/go-teaching-concepts.md` — mistake themes, decided games, human-policy framing, grading alternatives, puzzle variation design
- `references/html-build-guide.md` — lesson JSON schema (alternatives, wrong moves), HTML behaviour
