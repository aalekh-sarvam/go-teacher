# HTML Build Guide

For **report format 5 / lesson schema 2**, read [evidence-format5.md](evidence-format5.md) first. Its evidence, perspective and authoring contract supersedes the legacy fields below. Keep the game arc, stories, resources, praise and practice structure described here.

How the interactive HTML is structured, the lesson JSON schema, and the Go board renderer API. The `scripts/generate_lesson.py` script handles all of this automatically — this reference helps you write the lesson JSON that feeds into it.

## Lesson JSON schema

The generate script takes two inputs: the parsed JSON (from `parse_review.py`) and a lesson JSON (which you write). The lesson JSON structure:

```json
{
  "game_title": "Human vs AI (KataGo)",
  "players": {
    "black": "Human",
    "white": "AI (KataGo, 9d)"
  },
  "result": "W+48.5",
  "overall_feedback": "2-3 paragraphs of stylistic feedback about the player's game. What they do well, what their main weakness is, what phase needs the most work.",
  "progress": {
    "summary": "Optional, only when the report has a 'Compared with the student's earlier games' section: 1-2 sentences on the trend.",
    "rows": [{"metric": "Mean point loss per move", "this_game": "3.70", "recent_average": "4.20"}]
  },
  "game_arc": {
    "intro": "1-2 sentences framing the game's overall story: what kind of game it was and where it was decided.",
    "phases": [
      {
        "phase": "Opening",
        "move_range": "1-14",
        "narrative": "2-4 sentences: what each side was trying to do, the one or two moves that mattered most, and how the strategic balance shifted. Anchored in move numbers.",
        "turning_points": ["Move 12: White's shoulder hit pushed Black's extension back — the right side was White's from here on."]
      },
      {
        "phase": "Middle game",
        "move_range": "15-42",
        "narrative": "..."
      },
      {
        "phase": "Endgame",
        "move_range": "43-61",
        "narrative": "..."
      }
    ]
  },
  "lessons": [
    {
      "title": "Short descriptive title",
      "concept": "direction_of_play",
      "concept_label": "Direction of Play",
      "move_number": 15,
      "played_move": "E6",
      "preferred_move": "E7",
      "point_loss": 7.3,
      "winrate_before": "0.3%",
      "winrate_after": "0.1%",
      "score_before": "W+7.5",
      "score_after": "W+14.7",
      "explanation": "Detailed explanation for a beginner. Why is the preferred move better? What strategic concept was violated? Use concrete language.",
      "story": "2-4 sentences tracing the cause->effect chain around this move: how the position arose (the 4-6 moves before it) and what the mistake led to later in the game. This is the context around the tactical explanation, not a repeat of it. Shown as 'How we got here' above the explanation panel.",
      "variation_explanation": "What happens in KataGo's preferred variation. Walk through the PV and explain each significant move.",
      "principle": "One-sentence takeaway the beginner can remember.",
      "player_color": "B",
      "played_quality": "big mistake",
      "theme": "life and death",
      "level_framing": "The 10k model assigns the played move 30%; the 3k model assigns it much less and prefers D4 (60%). Explain the supported reason separately.",
      "refutation": ["F2", "D8", "G2", "C4"],
      "refutation_explanation": "2-3 sentences walking the numbered punishment: after 1 (F2) the D3 stones have one liberty; 3 (G2) captures them...",
      "better_line": ["D4", "G4", "C4", "E2"],
      "alternatives": [
        {"move": "F2", "loss_vs_best": 9.8, "quality": "big mistake", "explanation": "Saves one stone but leaves the D4 cut, so White captures three stones instead."},
        {"move": "C4", "loss_vs_best": 1.2, "quality": "good", "explanation": "Also connects; slightly slower than D4 because it leaves White the E2 hane."}
      ]
    }
  ],
  "puzzles": [
    {
      "title": "Puzzle title",
      "concept": "direction_of_play",
      "concept_label": "Direction of Play",
      "board_size": 9,
      "black_stones": ["C3", "D3", "C4"],
      "white_stones": ["G6", "G7", "H6"],
      "opponent_last_move": "H6",
      "player_to_move": "B",
      "correct_moves": ["G3"],
      "hint": "Look at where White is strong and where the board is open.",
      "explanation": "Explanation of why the correct move is right.",
      "wrong_moves": [
        {"move": "F5", "quality": "mistake", "loss_vs_best": 4, "refutation": ["F6", "E5", "E6"], "explanation": "Too close to White's wall: White presses at F6 (1) and Black's stones are squeezed against strength."},
        {"move": "E3", "quality": "inaccuracy", "loss_vs_best": 2, "refutation": ["G3"], "explanation": "Right direction but too slow; White takes G3 (1) and claims the side."}
      ],
      "generic_wrong_explanation": "Ask where White is strong (upper right) and where the board is still open."
    }
  ],
  "good_moves": [
    {
      "move_number": 21,
      "move": "G8",
      "explanation": "This move secured the corner while keeping sente. A strong player would recognize this as the largest point on the board."
    }
  ],
  "concepts_learned": [
    {
      "term": "Direction of Play",
      "japanese": "方向 (Hōkō)",
      "description": "The concept of choosing which side of the board to develop based on the relative strength of stones — play away from opponent strength, toward open areas.",
      "resources": [
        {"title": "Direction of Play — Sensei's Library", "url": "https://senseis.xmp.net/?DirectionOfPlay"},
        {"title": "Go Magic: Fundamentals of Go", "url": "https://gomagic.org/courses/the-fundamentals-of-go-on-13x13/"}
      ],
      "anecdote": "Go Seigen, one of the greatest players of the 20th century, revolutionized Go theory in the 1930s by emphasizing the importance of playing on the open side. His 'new fuseki' approach challenged the traditional focus on fixed corner patterns."
    }
  ]
}
```

### Field reference

**Top level:**
- `game_title`, `players`, `result` — from the parsed JSON
- `overall_feedback` — your stylistic assessment (2-3 paragraphs)
- `progress` — optional top-level object: `summary` text and `rows` (`metric`, `this_game`, `recent_average`) copied from the parsed `history.metrics`. Rendered as a "Your Progress" card after the game arc. Omit when the report has no history section.
- `game_arc` — required: the phase-by-phase narrative. An object with `intro` (1–2 sentences) and `phases`, one entry each for the phases the game actually had (typically Opening / Middle game / Endgame — a very short game may warrant only two, or even one). Each entry: `phase` (display name), `move_range` (e.g. "1–14"), `narrative` (2–4 sentences; blank-line-separated paragraphs allowed), optional `turning_points` (array of short move-tagged notes), and an optional integer `anchor_move` — the move whose position the phase card jumps the context board to (default: the end of the phase's move range; a phase whose range can't be parsed renders as a plain, non-clickable card). Rendered as a "How the Game Unfolded" section between the Game Overview and the lessons, with one card per phase and turning points as a bulleted list. The generator skips the section only when the field is missing (backwards compatibility with lesson JSON written before this field existed) — never rely on that. Follow the arc methodology and board-fact discipline in `references/game-arc-commentary.md`.
- `lessons` — array of 2-3 lesson objects
- `good_moves` — array of 1-3 moves the player played well (shown before puzzles)
- `puzzles` — array of 2-4 puzzle objects (1-2 per lesson concept)
- `concepts_learned` — array of Go concepts covered, with Japanese terms, resources, and anecdotes (shown after puzzles)

**Lesson object:**
- `theme` — the candidate's theme from the report (shown as a badge next to the concept label).
- `level_framing` — optional: the human-policy comparison ("At your level" callout above the principle). Use the report's human and target-profile numbers; omit when neither exists.
- `refutation` — (schema 1 only; schema 2 takes it from the report) the opponent's punishing sequence after the played move, opponent first, GTP coordinates. Shown in the **Your move and what it allowed** panel as numbered stones, with the coloured sequence line ("B G4 → W F3 → …") under the stepper. `refutation_explanation` is the text shown with it; refer to the stone numbers or the coloured tokens, never bare coordinates.
- `better_line` — (schema 1 only) KataGo's line after the preferred move, mover first. Shown in the **A better plan** panel with numbered stones and `variation_explanation` as its text.
- `player_color` — "B" or "W": the student's colour for this lesson. Optional: when omitted the generator uses the colour of that move in the parsed move list, then `game_info.student`. Set it explicitly when the student is White.
- `played_quality` — quality label of the played move (best / good / inaccuracy / mistake / big mistake / blunder).
- `alternatives` — (schema 1 only; schema 2 uses `alternative_explanations` keyed by searched move) other moves a beginner might consider, each with `move`, `loss_vs_best` (points, from the candidate table), `quality`, and an `explanation`. Shown in the **Other choices, graded** panel: every alternative is circled on the board with a colour for its quality, and the branch selector lets the student step through each evaluated branch (the report's `tree_*` sequences, else the candidate's `pv`).
- `move_number` — which move in the game this lesson covers (1-indexed). The generate script replays all moves before this to reconstruct the board position.
- `played_move` / `preferred_move` — GTP coordinates (e.g., "E6", "E7"). The script uses the parsed JSON's move list for the full context.
- `explanation` — (schema 1) the main teaching text of the **Your move** panel; schema 2 lessons write `panels.played` instead
- `story` — required on every lesson: 2–4 sentences tracing the cause→effect chain around this move (how the position arose and what the mistake led to), anchored in move numbers. Rendered as a "How we got here" block at the top of the lesson side panel, always visible regardless of which navigation button is active. Keep it distinct from `explanation` (the tactical why) — the story is the game-flow context. If a chain genuinely doesn't exist (the mistake came out of nowhere), write that: "This one came out of nowhere — a rare unforced error" still gives the student the arc context.
- `variation_explanation` — (schema 1) the text of the **A better plan** panel; schema 2 lessons write `panels.best` instead
- `principle` — one-sentence takeaway, displayed prominently. Like all static prose it goes through the `**bold**` formatter and the glossary (see below).
- `panels` — (schema 2) `{position, played, best, alternatives, local, what_if}` prose for the six panels. Panel prose uses the same rules as static prose: `**bold**` becomes `<strong>`, blank lines separate paragraphs, HTML is escaped, and the first glossary term in the panel is defined with `<abbr>`. The pairing text of a `sequence_explanations[id]` entry replaces the panel prose while that sequence is selected.
- `story`, `level_framing` — see below; both go through the same formatter and glossary.

**Puzzle object:**
- `black_stones` / `white_stones` — arrays of GTP coordinates for the initial position
- `opponent_last_move` — GTP coordinate of the opponent's last move (the move that created the position the student is responding to). Shown as a blue square on the board so the student sees what the opponent just did. Always include this — it gives the student the context they need to identify the right response.
- `player_to_move` — "B" or "W"
- `correct_moves` — array of accepted correct answers (usually 1, sometimes 2)
- `hint` — text shown when user clicks "Hint"
- `explanation` — shown after a correct answer and in the evaluations overlay
- `wrong_moves` — 2–3 tempting wrong answers, each `{move, quality, loss_vs_best, refutation, explanation}`. Clicking one shows its quality badge and explanation, and plays `refutation` (the full alternating continuation, opponent first, GTP coordinates) on the board with numbered stones. The validator replays captures, passes and ko to check each successive move; the strongest defence still needs verification.
- `generic_wrong_explanation` — retained for compatibility. Unlisted clicks are now labelled unverified rather than wrong.

**Good move object (praise card):**
- `move_number` — which move in the game (the board replays all moves up to and including this one)
- `move` — GTP coordinate of the good move (shown with a green circle marker). The card title is colour-safe: "Move 21: B G8".
- `explanation` — why this move was good, in beginner-friendly language (`**bold**` and the glossary apply)
- Evidence fields are **hydrated from the report**, not written by you: when `teaching.praise[]` in parsed.json has an entry for the same `move_number`, the generator copies its `kind_label` (badge: capture / saved a group / only good move / non-obvious best move / best move), `note` (one factual line under the title), `stones_captured` (shown as "Captured N stones" when > 0), `gap` (shown as "N points better than the next searched move"), `rating` and `rating_source`. Report values override anything you wrote for those keys.
- `rating` should normally be omitted. A rating badge is rendered **only** together with a visible "Source: …" line taken from `rating_source`; a rating with no source (yours or the report's) is dropped silently. Do not invent a kyu/dan strength to encourage the student.
- Cards sit in a responsive grid (one column on phones, two to three on wide screens) so three or four praise cards read as a set.

**Concept learned object:**
- `term` — the English name of the Go concept (e.g., "Direction of Play", "Sente and Gote", "Shape")
- `japanese` — the Japanese term with kanji and romaji (e.g., "方向 (Hōkō)", "先手 (Sente)"). Include the kanji, the romaji reading, and optionally the English translation.
- `description` — a clear, beginner-friendly explanation of the concept (2-3 sentences)
- `resources` — array of `{title, url}` objects linking to external learning resources. Include links to Sensei's Library, Go Magic, YouTube videos, or other established Go resources. Search the web to find the best links.
- `anecdote` — an interesting historical story or Go proverb related to the concept. Make it memorable — stories about famous players (Go Seigen, Shusaku, Lee Sedol), famous proverbs ("Play on the open side"), or historical moments in Go. Keep it 2-4 sentences.

## Fields the generator reads from parsed.json

Besides `game_info`, `moves`, `move_details` and `teaching.candidates` (joined by `lesson_contract.hydrate`), the generator consumes:

- `student_profile.estimate.{rank_label, low_label, high_label, wording}` — perspective-bar "Your level" label and small wording text
- `student_profile.working_rank.{rank_label, games}` — "Working rank" line in Your Progress
- `student_profile.profiles_used.{student, target}` (fallback `profiles`) — target label and the level fallback; named in the source line
- `teaching.praise[]` — `move_number`, `mv`, `kind_label`, `note`, `gap`, `stones_captured`, `rating`, `rating_source` for the praise cards
- `teaching.candidates[].refutation_with_colours` / `better_line_with_colours`, every sequence's `moves_with_colours`, and `move_details[n].candidates[].pv` / `pv_with_colours` — colour-safe sequence lines

## How the generated HTML works

### Go board renderer

The HTML embeds a lightweight canvas-based Go board renderer in vanilla JavaScript (no external dependencies). It handles:

- Grid drawing with star points
- Stone placement (black/white) with shadows
- Move replay with capture handling (for lesson positions)
- Explicit stone placement (for puzzle positions)
- Visual markers: blue square (opponent's last move), coloured circles (quality colours for alternatives), numbered labels for sequence stones (numbers disappear with captured stones)
- Click detection (for puzzle answering)

### Context board

The Game Overview and How the Game Unfolded sections sit in a two-column layout with a persistent board pinned beside the prose (it scrolls away before "Your Progress", where the lesson boards take over; on narrow screens it stacks above the text and stays pinned at reduced size). Clicking a phase card jumps the board to that phase's `anchor_move` and highlights the card; clicking a move reference in the prose (a dotted-underlined "move N" or a coordinate that was played exactly once in the game) shows the position after that move with the move circled and a caption naming it. The overview defaults to the final position (`overview_anchor_move` overrides it).

### Perspective bar and level labels

A sticky bar above the lessons holds three perspective buttons: **Engine**, **Your level · X** and **Target level · Y**. They select which sampled sequences the panels show (`played_engine` / `played_student` / `played_target`, and so on).

Where the labels come from (all read from parsed.json, never from the lesson JSON):

- **Your level** — `student_profile.estimate.rank_label` when the report has an estimate; the estimate's `wording` ("plays like a 12k in this game (range 14k–10k, 40 moves)") appears as small text under the bar's heading. Without an estimate the label falls back to `student_profile.profiles_used.student`, then to the legacy top-level `profiles.student`.
- **Target level** — `profiles_used.target` (fallback `profiles.target`).
- One small source line is always printed: either "Your level is this game's human-profile estimate (12k, range 14k–10k); the human-style sequences were sampled with the 10k profile and the target level is the 3k profile…", or "Levels are the human profiles used for this analysis (student 10k, target 3k); no per-game level estimate was available.", or "No level estimate is available for this report". The estimate describes similarity to human play; the line says so explicitly.
- **Your Progress** card — when `student_profile.working_rank` exists, the card ends with "Working rank: 11k" and a source line ("smoothed over your last N analysed games"). The card is rendered whenever there is progress prose, progress rows, or a working rank.

### Lesson panels

Each lesson has a board with a sequence stepper on the left and, on the right, the "How we got here" story, six panel buttons, an optional branch selector, the prose area, a facts block, the optional "At your level" callout and the principle. The six panels, each labelled with a one-line "what this shows":

| Panel key | Label | What it shows |
|---|---|---|
| `position` | Position | The board just before the move; facts: best play versus passing, difficulty label and near-best count |
| `played` | Your move and what it allowed | The played move and the opponent's strongest replies (`played_<level>`, plus `opponent_refutation` in human perspectives) |
| `best` | A better plan | The engine's preferred move and its continuation (`best_<level>`); facts: initiative class, ownership differences by region |
| `alternatives` | Other choices, graded | Every searched alternative circled by quality; the selector steps through each evaluated branch (`tree_*`, else the candidate `pv`) |
| `local` | Read the local fight | The restricted local search: group, liberties, who-moves-first trials and the caveat |
| `what_if` | What would likely happen next | Longer sampled continuations after the played and the better move, with the rollout comparison |

Panel prose comes from `panels.<key>` (schema 2) or the legacy `explanation` / `variation_explanation` fields; a `sequence_explanations[id]` entry replaces it while that sequence is selected, with a label saying so. Prose is rendered by the same formatter as the static page: escape, then `**bold**` → `<strong>`, blank-line paragraphs, first glossary term wrapped in `<abbr>`.

### Sequence stepper and colour-safe sequences

Under the board: first / previous / play / next / last buttons, a range slider, a "k / n moves" state line, and a **sequence line** that spells the selected sequence with colours — "B G4 → W F3 → B D7 → …" — highlighting the move currently on the board and dimming the moves not yet played. The same string ("Sequence: B G4 → W F3 → …") is the first entry of the facts block for every panel that has a sequence, and the branch selector labels use the same tokens.

Tokens come from the report's `moves_with_colours` (evidence version 2) when present; otherwise the generator's `renderSequence(seq)` alternates from `first_to_move`. Fallback lines built from a candidate's `refutation` / `better_line` use `refutation_with_colours` / `better_line_with_colours`, and alternative branches use the candidate's `pv_with_colours`. Nothing in the UI prints a bare coordinate list.

### Glossary

`generate_lesson.py` holds a `GLOSSARY` dict (semeai, nakade, sente, gote, tenuki, atari, ko, seki, kyūsho, aji, moyo, hane, tesuji). In each block of static prose — overview, arc intro/narratives/turning points, story, principle, level framing, progress summary, praise explanation — the first occurrence of each term is wrapped in `<abbr title="definition">` (dotted underline, tooltip). The panels apply the same dictionary at runtime, once per panel render. Matching is whole-word and case-insensitive; "kyusho" matches without the macron. Keep using the terms in prose; the glossary explains them so you do not have to.

### Puzzle interaction

Each puzzle shows a static board position with click handling. A wrong click that matches a `wrong_moves` entry shows that move's quality badge and explanation and plays its refutation with numbered stones; a click that was not evaluated is labelled "has not been checked in this exercise" rather than wrong. "Show evaluations" overlays the correct move (★) and all listed wrong moves with their losses and prints a comparison list. The opponent's last move is marked with a **blue square** so the student understands what they're responding to — this mirrors how the position would look in a real game. A legend below the board controls explains the markers (blue square = opponent's last move, green circle = correct answer).

1. User clicks on any intersection
2. If the clicked point matches a `correct_moves` entry: green circle + success message + explanation
3. If it matches a listed wrong move: quality-coloured circle + explanation; unlisted clicks are marked unverified
4. A "Hint" button reveals the hint text
5. A "Reset" button clears the feedback and lets the user try again (the blue square remains visible)

### HTML layout

```
┌─────────────────────────────────────┐
│  Header: game title, players, result │
├─────────────────────────────────────┤
│  Game overview, development, progress│  (collapsible <details>)
│  ┌──────────────────┐ ┌──────────┐  │
│  │ Game Overview     │ │ Context  │  │
│  │ How the Game      │ │ board    │  │
│  │ Unfolded (phases) │ │ (pinned) │  │
│  └──────────────────┘ └──────────┘  │
│  Your Progress (rows + working rank) │
├─────────────────────────────────────┤
│  Perspective bar: Engine / Your level│  (sticky; wording + source line)
│  · X / Target level · Y              │
├─────────────────────────────────────┤
│  Lesson 1                             │
│  ┌──────────┐  ┌─────────────────┐  │
│  │  Board   │  │  How we got here │  │
│  │  Stepper │  │  Six panels      │  │
│  │  Sequence│  │  Branch selector │  │
│  │  line    │  │  Prose + facts   │  │
│  │  Legend  │  │  Principle       │  │
│  └──────────┘  └─────────────────┘  │
├─────────────────────────────────────┤
│  Lesson 2, 3 (same layout)           │
├─────────────────────────────────────┤
│  Moves You Played Well (grid)         │
│  ┌────────────┐ ┌────────────┐      │
│  │ Board      │ │ Board      │      │
│  │ Move · kind│ │ Move · kind│      │
│  │ note, facts│ │ note, facts│      │
│  │ explanation│ │ explanation│      │
│  │ rating +   │ │            │      │
│  │ Source     │ │            │      │
│  └────────────┘ └────────────┘      │
├─────────────────────────────────────┤
│  Puzzle 1, 2 …                        │
│  ┌──────────┐  ┌─────────────────┐  │
│  │  Board   │  │  Goal, source    │  │
│  │  Hint /  │  │  Feedback        │  │
│  │  Evals / │  │  Hint box        │  │
│  │  Reset   │  │                  │  │
│  └──────────┘  └─────────────────┘  │
├─────────────────────────────────────┤
│  Go Concepts & Resources              │
├─────────────────────────────────────┤
│  Footer                               │
└─────────────────────────────────────┘
```

The page is responsive and uses a warm wooden board color (#dcb35c) with clean typography. All CSS is inline in a `<style>` tag. All JavaScript is inline in a `<script>` tag. No external resources are loaded — the file works completely offline.