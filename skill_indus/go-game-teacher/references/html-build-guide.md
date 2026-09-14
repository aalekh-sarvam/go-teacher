# HTML Build Guide

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
      "level_framing": "At 10k this move is played 30% of the time; at 3k almost never — a 3k plays D4 (60%) because it saves the D3 stones first.",
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
      "rating": "2 kyu",
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
- `game_arc` — required: the phase-by-phase narrative. An object with `intro` (1–2 sentences) and `phases`, one entry each for the phases the game actually had (typically Opening / Middle game / Endgame — a very short game may warrant only two, or even one). Each entry: `phase` (display name), `move_range` (e.g. "1–14"), `narrative` (2–4 sentences; blank-line-separated paragraphs allowed), and optional `turning_points` (array of short move-tagged notes). Rendered as a "How the Game Unfolded" section between the Game Overview and the lessons, with one card per phase and turning points as a bulleted list. The generator skips the section only when the field is missing (backwards compatibility with lesson JSON written before this field existed) — never rely on that. Follow the arc methodology and board-fact discipline in `references/game-arc-commentary.md`.
- `lessons` — array of 2-3 lesson objects
- `good_moves` — array of 1-3 moves the player played well (shown before puzzles)
- `puzzles` — array of 2-4 puzzle objects (1-2 per lesson concept)
- `concepts_learned` — array of Go concepts covered, with Japanese terms, resources, and anecdotes (shown after puzzles)

**Lesson object:**
- `theme` — the candidate's theme from the report (shown as a badge next to the concept label).
- `level_framing` — optional: the human-policy comparison ("At your level" callout above the principle). Use the report's human and target-profile numbers; omit when neither exists.
- `refutation` — the opponent's punishing sequence after the played move, opponent first, GTP coordinates (copy the report's `refutation`, 4–8 moves). Rendered by the **What it allows** button: the board shows the played move (red circle) and then the sequence with numbered stones. `refutation_explanation` is the text shown with it; refer to the stone numbers.
- `better_line` — KataGo's line after the preferred move, mover first (copy `better_line`). Rendered by the **Better line** button with numbered stones and `variation_explanation` as its text.
- `player_color` — "B" or "W": the student's colour for this lesson. Optional: when omitted the generator uses the colour of that move in the parsed move list, then `game_info.student`. Set it explicitly when the student is White.
- `played_quality` — quality label of the played move (best / good / inaccuracy / mistake / big mistake / blunder).
- `alternatives` — 2–4 other moves a beginner might consider, each with `move`, `loss_vs_best` (points, from the candidate table), `quality`, and an `explanation` of why it is worse or nearly as good. Rendered as buttons under the board; clicking one places the stone with a colour for its quality and shows the explanation. A "Show all candidates" button overlays every listed move at once with its loss.
- `move_number` — which move in the game this lesson covers (1-indexed). The generate script replays all moves before this to reconstruct the board position.
- `played_move` / `preferred_move` — GTP coordinates (e.g., "E6", "E7"). The script uses the parsed JSON's move list for the full context.
- `explanation` — the main teaching text (shown when user clicks "Show played move")
- `story` — required on every lesson: 2–4 sentences tracing the cause→effect chain around this move (how the position arose and what the mistake led to), anchored in move numbers. Rendered as a "How we got here" block at the top of the lesson side panel, always visible regardless of which navigation button is active. Keep it distinct from `explanation` (the tactical why) — the story is the game-flow context. If a chain genuinely doesn't exist (the mistake came out of nowhere), write that: "This one came out of nowhere — a rare unforced error" still gives the student the arc context.
- `variation_explanation` — explains what happens when the preferred move is played (shown when user clicks "Show better move")
- `principle` — one-sentence takeaway, displayed prominently

**Puzzle object:**
- `black_stones` / `white_stones` — arrays of GTP coordinates for the initial position
- `opponent_last_move` — GTP coordinate of the opponent's last move (the move that created the position the student is responding to). Shown as a blue square on the board so the student sees what the opponent just did. Always include this — it gives the student the context they need to identify the right response.
- `player_to_move` — "B" or "W"
- `correct_moves` — array of accepted correct answers (usually 1, sometimes 2)
- `hint` — text shown when user clicks "Hint"
- `explanation` — shown after a correct answer and in the evaluations overlay
- `wrong_moves` — 2–3 tempting wrong answers, each `{move, quality, loss_vs_best, refutation, explanation}`. Clicking one shows its quality badge and explanation, and plays `refutation` (the opponent's 1–4 punishing replies, opponent first, GTP coordinates) on the board with numbered stones. Every refutation move must be an empty point after the wrong move is placed; the generator does not validate this.
- `generic_wrong_explanation` — shown for clicks that match none of the listed moves

**Good move object:**
- `move_number` — which move in the game (the board replays all moves up to and including this one)
- `move` — GTP coordinate of the good move (shown with a green circle marker)
- `rating` — a fun kyu/dan rating estimate (e.g., "3 kyu", "1 dan") shown as a badge. This is subjective and meant to be encouraging — estimate what level of player would typically find this move.
- `explanation` — why this move was good, in beginner-friendly language

**Concept learned object:**
- `term` — the English name of the Go concept (e.g., "Direction of Play", "Sente and Gote", "Shape")
- `japanese` — the Japanese term with kanji and romaji (e.g., "方向 (Hōkō)", "先手 (Sente)"). Include the kanji, the romaji reading, and optionally the English translation.
- `description` — a clear, beginner-friendly explanation of the concept (2-3 sentences)
- `resources` — array of `{title, url}` objects linking to external learning resources. Include links to Sensei's Library, Go Magic, YouTube videos, or other established Go resources. Search the web to find the best links.
- `anecdote` — an interesting historical story or Go proverb related to the concept. Make it memorable — stories about famous players (Go Seigen, Shusaku, Lee Sedol), famous proverbs ("Play on the open side"), or historical moments in Go. Keep it 2-4 sentences.

## How the generated HTML works

### Go board renderer

The HTML embeds a lightweight canvas-based Go board renderer in vanilla JavaScript (no external dependencies). It handles:

- Grid drawing with star points
- Stone placement (black/white) with shadows
- Move replay with capture handling (for lesson positions)
- Explicit stone placement (for puzzle positions)
- Visual markers: colored circles (red for played move, green for preferred move, quality colours for alternatives), labels
- Click detection (for puzzle answering)

### Lesson interaction

Each lesson shows a Go board with three navigation states:

1. **Position** — the board before the move (moves replayed from game start). The opponent's last move is marked with a **blue square** so the student can see what the opponent just did — this provides the context for understanding why their own move was a mistake.
2. **Show played move** — the same position with the played move added (red circle marker). The blue square on the opponent's last move remains visible.
3. **Show better move** — the same position with KataGo's preferred move added (green circle marker). The blue square remains visible here too.

The explanation text changes based on which button is active. The principle is always visible below, and the "How we got here" story block (`story`), when present, is always visible above the explanation panel.

A legend below the board controls explains the three marker types: blue square (opponent's last move), red circle (your move), green circle (KataGo's recommendation).

### Sequence playback on lesson boards

**What it allows** places the played move (red circle) and then the lesson's `refutation` with numbered stones, alternating colours starting with the opponent; the side panel shows `refutation_explanation`. **Better line** plays `better_line` from the base position starting with the student's colour and shows `variation_explanation`. Both buttons appear only when the corresponding array is non-empty.

### Alternatives on lesson boards

Under the three navigation buttons, each listed alternative has its own button labelled with the move and its loss ("F2 (-9.8)"). Clicking places that stone, colours the marker by quality (green best → yellow inaccuracy → orange mistake → red blunder) and shows the alternative's explanation with a quality badge. "Show all candidates" overlays the best move (★), the played move and every alternative with their losses, so the student sees the whole spectrum at once.

### Puzzle interaction

Each puzzle shows a static board position with click handling. A wrong click that matches a `wrong_moves` entry shows that move's quality badge and explanation; other wrong clicks show `generic_wrong_explanation`. "Show evaluations" overlays the correct move (★) and all listed wrong moves with their losses and prints a comparison list. The opponent's last move is marked with a **blue square** so the student understands what they're responding to — this mirrors how the position would look in a real game. A legend below the board controls explains the markers (blue square = opponent's last move, green circle = correct answer).

1. User clicks on any intersection
2. If the clicked point matches a `correct_moves` entry: green circle + success message + explanation
3. If it doesn't match: red circle + "Try again" message
4. A "Hint" button reveals the hint text
5. A "Reset" button clears the feedback and lets the user try again (the blue square remains visible)

### HTML layout

```
┌─────────────────────────────────────┐
│  Header: game title, players, result │
├─────────────────────────────────────┤
│  Game Overview (overall feedback)    │
├─────────────────────────────────────┤
│  How the Game Unfolded (game arc)    │
│  Intro + one card per phase           │
│  (Opening / Middle / Endgame)         │
├─────────────────────────────────────┤
│  Lesson 1                             │
│  ┌──────────┐  ┌─────────────────┐  │
│  │  Board   │  │  How we got here │  │
│  │  Canvas  │  │  Navigation      │  │
│  │          │  │  Explanation     │  │
│  │          │  │  Principle        │  │
│  └──────────┘  └─────────────────┘  │
├─────────────────────────────────────┤
│  Lesson 2 (same layout)              │
├─────────────────────────────────────┤
│  Lesson 3 (same layout)              │
├─────────────────────────────────────┤
│  Moves You Played Well                │
│  ┌──────────┐  ┌─────────────────┐  │
│  │  Board   │  │  Rating badge    │  │
│  │  Canvas  │  │  Explanation     │  │
│  └──────────┘  └─────────────────┘  │
├─────────────────────────────────────┤
│  Puzzle 1                             │
│  ┌──────────┐  ┌─────────────────┐  │
│  │  Board   │  │  Instructions    │  │
│  │  Canvas  │  │  Feedback        │  │
│  │          │  │  Hint | Reset    │  │
│  └──────────┘  └─────────────────┘  │
├─────────────────────────────────────┤
│  Puzzle 2 (same layout)              │
├─────────────────────────────────────┤
│  Go Concepts & Resources              │
│  ┌─────────────────────────────────┐ │
│  │  Term + Japanese + Description   │ │
│  │  Resource links                  │ │
│  │  Anecdote box                    │ │
│  └─────────────────────────────────┘ │
├─────────────────────────────────────┤
│  Footer                               │
└─────────────────────────────────────┘
```

The page is responsive and uses a warm wooden board color (#dcb35c) with clean typography. All CSS is inline in a `<style>` tag. All JavaScript is inline in a `<script>` tag. No external resources are loaded — the file works completely offline.