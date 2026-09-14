# Go Teacher v5 — Lesson page UI redesign (panels + Perspective) — REVISED FINAL

Scope: the HTML produced by `/Users/aalekhsharan/code/go_teacher/skill_indus/go-game-teacher/scripts/generate_lesson.py`, the lesson JSON the skill model writes, `scripts/parse_review.py`, `scripts/validate_lesson.py`, `SKILL.md` and `references/html-build-guide.md`. Anything the Rust app must emit for the panels is marked **[report dep]** and specified exactly (shape, algorithm, thresholds, cost) so the report/analysis engineer can implement it without asking. Nothing here touches the live view (`static/index.html`).

Single-file constraint unchanged: one HTML, inline CSS and JS, no fonts, no images, no fetches, Unicode glyphs only (◀ ▶ ⏮ ⏭ ★ ✓ ✗ ▲ ▼ ■ ●).

## What changed since the reviewed draft (for the implementer)

1. **Data path inverted.** The skill model no longer transcribes trees, rollouts, ownership lists, value rows or any score/visit/probability. Every engine-computed block lives in a per-candidate `evidence` JSON block in the report, copied verbatim by the parser into `parsed.teaching.candidates[k].evidence`; the generator joins panels to evidence by `move_number`. Lesson JSON panels carry **prose only** (`summary`, `text`, `note`). Numeric fields the skill used to write are forbidden in schema 2 except report strings copied verbatim ("W+7.5", "0.3%") in legacy fields.
2. **Setup stones / handicap** handled end-to-end (report line → parser → generator → validator); first mover derived from the record.
3. **Human-move selection** fixed for White to move (`u_mover`), reads `humanPolicy` via `policy_index`; "most common move" (raw argmax, probability quoted) and "most likely line" (utility-weighted argmax) are distinct and named.
4. **Go semantics corrected:** status swing = value of playing first, from ownership, not scoreLead; "pass value" renamed *tempo value*, urgency measured locally; per-candidate worth = `pass_value − loss` stated once; sente/gote only for good moves with wider thresholds and no glyphs on 9x9; green circle only on genuinely recommended moves; realistic-refutation text splits the opponent's reply from the student's defence.
5. **Fewer, clearer cards:** ≤ 6 panels per lesson with a theme rule; `engine_only` kinds show no fallback line; per-panel view state is a visible in-card segmented control that resets on close; no `localStorage`; header capped at 4 chips in student perspective.
6. **Cost model re-done and tiered** (cheap items for every candidate, heavy items for ≤ 4 lessons), pure human-policy rollouts for the counterfactual, 6-profile rank estimation, a published 10-minute budget with a degrade order.
7. Renderer fixes: DPR click mapping, glyph-density rule, grey numbers for real game moves, ▲/▼ for plans, tree depth 2 by default, sparkline plots score, generator computes the counterfactual-vs-game comparison itself.
8. Puzzles: no invented probabilities; skill-authored human punishments are labelled "teacher's judgement"; optional engine-verified puzzle seeds transformed by the generator, never by the model.

---

## 0. Design summary (what changes for the student)

A lesson is a board plus a vertical list of **cards** (accordion, one open at a time). Clicking a card plays its sequence on the board with numbered stones, shows its text, and shows the numbers it rests on. A stepper under the board steps through the open card's sequence. One page-wide **Perspective** control (Engine | Your level | Target level) decides which variant each card plays by default; a card with more than one variant shows a small segmented control at the top of its body whose selected segment *is* the card's current view.

Panel kinds (fixed vocabulary, canonical order):

| # | kind | Card title | Board | Perspective |
|---|---|---|---|---|
| 1 | `played` | What happened | Your move (red circle), then the real game continuation in grey numbers | all (chips only) |
| 2 | `engine` | What KataGo prefers (missed opportunity: What was available) | Better move (green circle, 1) and KataGo's line; human variants = first 8 plies of the counterfactual | all |
| 3 | `allows` | What it allows (missed opportunity: How the opponent closed the door) | Your move, then the punishment: engine or realistic | all |
| 4 | `human` | What players actually play here | Overlay E / Y / T at step 0 always; the perspective selects which continuation plays | all |
| 5 | `alternatives` | Alternatives, graded | All candidates overlaid with losses; list rows; a row places its stone | `engine_only` unless continuations exist |
| 6 | `values` | How big is a move here | Per-candidate worth vs passing; S/G glyphs (not on 9x9) | `engine_only` |
| 7 | `plans` | Where the points go | ▲/▼ ownership difference best vs played, region rows | `engine_only` |
| 8 | `tree` | Explore the variations | Depth-2 (top lesson: 3) clickable tree | all (frequency chips + default path) |
| 9 | `counterfactual` | How the game would likely have gone | 20–30 plies of human-policy rollout after the better move, checkpoints + sparkline | student/target (engine optional) |
| 10 | `status` | Life and death here | Group ▲, allowed points hatched, attacker-first / defender-first lines | `engine_only` |

**Cap: 6 cards per lesson.** Core three (`played`, `engine`, `allows`); fourth `human` when profiles exist; then at most two *deep* cards chosen by theme (generator default, skill may override with `use`): life and death → `status`; endgame / counting → `values`; direction of play / over-defending / big move → `plans`; reading / tactics / cutting / connection → `tree`; any theme → `counterfactual` when the evidence has one, filling the last free slot. Anything beyond six collapses under one row "More detail" (a `<details>` element) — never dropped, never above the fold. The generator warns on stderr when it collapses.

Legacy lesson JSON (no `lesson_schema`) maps to `played`, `engine`, `allows`, `alternatives` and a text-only `human` (§9) with zero edits.

---

## 1. Page layout

```
┌─ sticky top bar (omitted when no profiles) ─────────────────────────────┐
│ Perspective: (●) Engine  ( ) Your level (10 kyu)  ( ) Target (3 kyu)    │
│ Click a card to play its line on the board; ◀ ▶ step through it.        │
└─────────────────────────────────────────────────────────────────────────┘
Header · Game Overview (+ Estimated level card) · How the Game Unfolded · Your Progress

Lesson N: title
[concept][theme]  big mistake · 12.1 pts worse | Score: you −7.5 → −14.7 | game already decided — judge by points | Played by 30% of 10 kyu · 2% of 3 kyu
┌───────────────────────────┐ ┌─────────────────────────────────────────────┐
│ board canvas (role=img)   │ │ How we got here (always visible)            │
│ ⏮ ◀  Step 3 of 8  ▶ ⏭     │ │ ▸ What happened      big mistake: 12.1 worse│
│ view badge                │ │ ▾ What it allows     punished: you −4.8     │
│ 1 F2 · 2 D8 · 3 pass …    │ │   [Engine | Your level | Target]  ← in-card │
│ legend (text + swatch)    │ │   text · numbers · note                      │
└───────────────────────────┘ │ ▸ What KataGo prefers  D3 keeps you at −7.5 │
                              │ ▸ …                                          │
                              │ Key principle                                │
                              └─────────────────────────────────────────────┘
Moves You Played Well · Puzzles (§7) · Go Concepts & Resources
```

- ≥ 900 px: two columns; board column `flex: 0 0 min(480px, 45%)`, `position: sticky; top: 56px`; card column `flex: 1; min-width: 300px`.
- 600–899 px: one column; the whole board block (canvas + stepper + chips) sticky at `top: 48px`, canvas `width: min(100vw − 32px, 400px)`.
- < 600 px: **only the canvas** is sticky, at `width: 60vw` (max 240 px, centred); the stepper sits directly under it inside the sticky block (one 36-px row); chips and legend scroll with the cards. Rationale: a 400-px board + chips left ~250 px for cards on a 700-px phone.
- Body side padding 16 px at every width; no horizontal scroll (tree and tables sit inside their own `overflow-x: auto`).
- Perspective bar `position: sticky; top: 0; z-index: 10`; omitted from the DOM when `profiles` has neither `student` nor `target`.

---

## 2. Data inputs

### 2.1 Evidence block **[report dep]** — the report emits, parser copies, generator consumes

In `## Teaching candidates`, each `### Candidate: move N (...)` block gains one fenced block:

````
```json evidence
{ ...one JSON object, ≤ 20 KB, Black-perspective numerics... }
```
````

`parse_review.py` copies it verbatim (`json.loads`) into `teaching.candidates[k].evidence`; a malformed block is a parser *warning* and the candidate gets `evidence: null` (legacy path). `Report format: 5`; `SUPPORTED_FORMATS = {2,3,4,5}`. Fields (all optional except `schema`, `move_number`, `mover`, `played`):

```jsonc
{
  "schema": 1, "move_number": 23, "mover": "B", "played": "A19", "best": "D3",
  "score_before": -7.5, "score_after": -14.7,            // Black perspective, points (rootInfo.scoreLead at turn i, i+1; deep values when two-pass)
  "winrate_before": 0.003, "winrate_after": 0.001,       // Black perspective 0..1
  "decided": true,                                       // winrate_before outside [0.05, 0.95]
  "loss": 12.1,                                          // mover's points, deep (the game's post-move loss)
  "kind": "mistake" | "missed_opportunity", "gain_available": 6.0,   // §6.6
  "phase": "middlegame",
  "tempo": {"pass_value": 9.2, "phase_typical": 5.5, "urgency_local": 7.0, "urgency": "urgent", "visits": 300},   // §6.4
  "sharpness": {"within_1pt": 1, "counted": 4, "policy_top": 0.62, "policy_entropy": 1.8, "label": "sharp", "findability": "obvious"},  // §6.3
  "candidates": [                                        // top 5 by visits from the (deep) pre-move search + the played move, mover's points
    {"move": "D3", "score": -7.5, "winrate": 0.004, "visits": 812, "loss": 0.0, "policy": 0.62,
     "human_freq": {"student": 0.40, "target": 0.60},   // raw humanPolicy of this move at each profile, 0..1
     "worth_pts": 9.2,                                   // pass_value − loss  (§6.5)
     "sente": {"class": "sente"|"gote"|"unclear"|null, "tenuki_cost": 5.1, "reply": "C2", "local": true, "visits": 300},  // null when loss ≥ 3 (§6.5)
     "pv": ["D3","C3","C2","E4"]},
    {"move": "A19", "played": true, "loss": 12.1, "worth_pts": -2.9, ...}
  ],
  "better_line": {"sequence": ["D3","C3","C2","E4"], "score_after": -7.5, "visits": 812},
  "refutation": {                                        // after the played move, opponent first
    "engine":  {"sequence": ["F2","D8","G2","C4"], "score_after": -14.7, "visits": 1000, "source": "KataGo 1000 visits"},
    "student": {"sequence": ["F2","D9","G2","C3"], "score_after": -16.1, "opp_reply": "F2", "opp_reply_prob": 0.41,
                "opp_reply_delta": 0.0, "defence_delta": -1.4, "selection": "most_likely", "profile": "rank_10k_8k", "visits_per_ply": 100},
    "target":  {...}
  },
  "human": {                                             // "what players actually play here"
    "student": {"top_move": "E2", "prob": 0.30, "continuation": {"sequence": ["E2","D2","F3"], "score_after": -9.1, "selection": "most_likely"}},
    "target":  {"top_move": "D3", "prob": 0.60, "continuation": null}     // null when top_move == best (dedupe, §4 human)
  },
  "tree": { ... §5 ... },
  "plans": {"vs": "best", "best_visits": 812, "points": [{"point": "D3", "delta": -0.8}, ...], "regions": [{"region": "lower left", "delta": -5.2}, ...]},  // §4 plans
  "status": { ... §4 status ... },
  "counterfactual": {
    "student": {"sequence": [...25 plies...], "first_color": "B", "method": "human_policy_argmax_1visit", "profile": "rank_10k_8k",
                "checkpoints": [{"ply": 5, "score": -3.1, "winrate": 0.35}, {"ply": 10, ...}], "final": {"ply": 25, "score": -2.0, "winrate": 0.41}},
    "target": {...}, "engine": null
  },
  "profiles": {"student": "rank_10k", "target": "rank_3k", "opponent": "rank_8k" | "engine"},
  "timing_s": {"tempo": 3.1, "sente": 9.0, "tree": 6.4, "refutation": 28.0, "human": 0.0, "counterfactual": 25.2, "status": 6.1}
}
```

Sign convention, stated once: **every `score*`/`winrate*` is Black's perspective; every `loss*`, `worth_pts`, `tenuki_cost`, `pass_value`, `urgency_local`, `gain_available`, `*_delta` is in the points of the player named by the nearest `mover`/`player` field (candidates: the lesson's mover; tree nodes: `node.mover`; refutation deltas: the student).** The generator converts to "you" once (`sign = student == 'B' ? +1 : −1`).

### 2.2 Game information additions **[report dep]**

| Setup stones | Black: D4 Q16 D16 Q4; White: none |
| First to move | White |
| Human profiles | student rank_10k (10 kyu); target rank_3k (3 kyu); opponent engine (KataGo bot) |
| Rank estimate | overall 9 kyu (band 8–10 kyu) from 142 moves; opening 7 kyu (16 moves), middlegame 9 kyu (78), endgame 11 kyu (48) |

Parser: `game_info.setup_black: []`, `setup_white: []`, `first_to_move: "B"|"W"` (default: White if only Black setup stones, else Black — same rule as `GameRecord::who_moves_first`), `game_info.profiles: {student: {profile, label}, target: {...}, opponent: {profile, label}}`, `game_info.rank_estimate: {overall, band: [lo, hi], moves, by_phase: [{phase, rank, moves}]}` (labels as strings "9 kyu"; the parser also stores `rank_num` on a stone scale: 20k→0 … 1k→19, 1d→20 … 9d→28). Parser also adds `score_after_num` / `winrate_after_num` to every compact move (`parseLead`, §6.1) so the page never parses strings at runtime for arithmetic.

### 2.3 Lesson JSON schema 2 (what the skill writes)

```jsonc
{
  "lesson_schema": 2,
  "game_title": "...", "players": {...}, "result": "...", "overall_feedback": "...", "progress": {...}, "game_arc": {...},   // unchanged
  "rank_estimate_text": "One or two sentences on what the estimate means for this student.",   // optional; numbers come from parsed.game_info.rank_estimate
  "lessons": [ Lesson ], "good_moves": [...], "puzzles": [ Puzzle ], "concepts_learned": [...]
}

Lesson = {
  "title": "Save the stones",  "move_number": 23,  "played_move": "A19",       // required; played_move must equal the record
  "player_color": "B",                                                          // optional, resolved as today
  "concept": "life_and_death", "concept_label": "Life and Death", "theme": "life and death",
  "story": "...", "principle": "...",                                           // always visible
  "panels": [ Panel ],
  "follow_game_moves": 6,                                                       // 0–12; default 6 for schema 2, 0 for legacy
  // legacy v1 fields still accepted and mapped (§9); in schema 2 any numeric score/winrate/loss written by the skill is a validator PROBLEM
}

Panel = {
  "kind": "allows",                                   // fixed vocabulary; unknown → generic card
  "use": true,                                        // false drops a panel the generator would otherwise build from evidence
  "title": "optional override",
  "summary": "≤ 90 chars; optional — generator template used when absent (§8.1)",
  "text": {"engine": "...", "student": "...", "target": "..."} | "one text for all variants",   // paragraphs by blank line, **bold** allowed
  "note": "one sentence, italic",
  "rows": {"F2": "Saves one stone but leaves the D4 cut…", "C4": "Also connects; slower…"},    // alternatives only: prose per candidate move
  "data": "inline",                                   // opt-out: this panel carries its own variants/extras (legacy, hand-written, or when evidence is absent)
  "variants": {...}, "alternatives": [...], ...       // allowed only with data:"inline"; shapes as in §9 legacy mapping
}
```

Rules enforced by the validator (§10): a schema-2 panel of kind `tree|plans|values|status|counterfactual|human|allows|engine` with inline data while `evidence` exists for that move is a PROBLEM (drift); `text.student`/`text.target` may exist only if the evidence has that variant (warning otherwise: text without a line). The skill never writes coordinates except in legacy fields, puzzles and prose.

---

## 3. Board and sequence semantics

### 3.1 Base position and marks
- Base board = `GoBoard.placeStones(setupStones.black, setupStones.white)` then `replayMoves(allMoves, move_number − 1)`. `const setupStones = {...}` is emitted from `game_info.setup_*`. Same in `initGoodMove` and in the validator's `replay` and `play_sequence`. Move 1 in a handicap game has no opponent last move; the blue square is omitted. Alternation is never assumed from Black: the colour of each real move comes from `allMoves[i].color`; sequence colours alternate from the variant's `first_color`.
- Opponent's last move: blue square ■ `#24a`. Played move: red circle `#d9482b`, no number.
- **Green circle `#2a7a2a` only on move 1 of `engine` and `counterfactual`** (both start with the better move). `human` and `alternatives` sequences start with a circle in the panel colour (purple `#6a4c93` for human, the quality colour for alternatives); legend: "purple: the move a 10 kyu usually plays". When the human move equals the engine move the variant is dropped and the card says so (§4 human).
- Numbered stones: white text on black stones, black on white, 2-px halo of the opposite colour (`strokeText` then `fillText`), font `bold ${Math.floor(cell*0.42)}px system-ui`. **Real game moves** (the `played` card's continuation) use grey fill `#9a9a9a` with the same halo, and the legend item "grey numbers: what was actually played next".
- Labels attach to *stones*: after replaying to the current step, a number is drawn only if the point still holds a stone of the colour that played it. Captured stones appear struck through in the chips ("~~3 D8~~ captured by 6") and in the transcript. A recaptured point shows the latest number; chips keep both.
- `pass`: no stone; chip "3 pass"; counts as a step.
- Illegal entry (occupied / bad coordinate — evidence sequences are engine-legal by construction, so this only happens with inline or legacy data): skip the stone, chip in red with title "D4 is occupied — check the lesson JSON", continue.

### 3.2 Steps
```
steps = [{kind:'base'}]; if (variant.after_played) steps.push({kind:'played'});
sequence.forEach((m,k) => steps.push({kind:'seq', index:k}));
```
Overlay markers show at the base/played steps only. Default step on open: `played` → the played step (the mistake first, aftermath on demand); `alternatives`, `values`, `plans`, `tree`, `human` → their overlay step 0; `engine`, `allows`, `status`, `counterfactual`, generic → **last step** (one click shows the whole line). Counterfactual stones with sequence index > 12 are drawn with number opacity 0.55 so a 25-ply line stays readable; the sparkline marks the current step. Controls: ⏮ ◀ ▶ ⏭, "Step 3 of 8", and, when on a sequence stone, "3: White D8". Buttons disabled at the ends, never hidden.

### 3.3 Overlay markers
`overlay[]` entries are built by the generator: `{move, label?, loss?, quality?, tag?, glyph?, shade?}`.
- Circle colour from `quality` or `qualityFromLoss(loss)`.
- Board text = `label`, else ★ when `loss < 0.05`, else `−n` (one decimal when `cell ≥ 30` CSS px and loss < 10, integer otherwise). `tag` (E/Y/T/S/G) is a superscript in the circle's top-right.
- **Glyph-density rule:** when `cell < 24` CSS px (19x19 at ≤ 460 px), each marker shows exactly one glyph: the number for `alternatives`/`values`, the tag for `human`, only colour + ▲/▼ for `plans`; everything else lives in the list rows (which always carry the full text). Overlay markers are capped at 8 on 19x19 (best, played, ≤ 5 candidates, human tops merged) and 12 on 13x13; extra points appear only in the rows.
- `plans` markers: `glyph: "▲"|"▼"` drawn in the square with a translucent tint (alpha `0.15 + 0.45·|delta|`, black for ▲ = "the better move keeps this for you", white for ▼), never colour alone.
- Clicking a marked point (lesson boards get a click handler only while an overlay is showing) selects that marker: `alternatives` → its row; `human` → the variant tagged Y/T (E → engine panel? no: E selects the `engine` view of the human card, i.e. the overlay); `tree` → the child node.

### 3.4 Legend (text + swatch, re-rendered per open card, only relevant items)
"■ opponent's last move · ● red: your move · ● green: KataGo's move · ● purple: the move a 10 kyu usually plays · numbered stones: this line, 1 = first move · grey numbers: what was actually played next · ★ best move · −n: points worse than the best move · ▲ the group being judged · hatched: the points KataGo was allowed to consider · S: your opponent has to answer next to this move (ignoring it would cost them at least 4 points) · G: your opponent is free to play elsewhere · ▲/▼ (plans): the better move keeps / gives up these points".

### 3.5 Canvas and click mapping
`LOGICAL = 720`. `canvas.width = canvas.height = LOGICAL * dpr` (dpr = `devicePixelRatio || 1`), `ctx.setTransform(dpr,0,0,dpr,0,0)`; CSS `width:100%; max-width:480px; aspect-ratio:1`. **`computeLayout` uses `LOGICAL`, never `canvas.width`; `toXY` uses `scale = LOGICAL / rect.width`.** (The existing `canvas.width / rect.width` would be off by dpr on retina.) `role="img"` with an `aria-label` set once per card open ("Lesson 1, move 23, showing What it allows, your level. Board before move 23, White to answer."). The **step label** is the only `aria-live="polite"` region ("Step 3 of 8: White D8"); the chips `<ol>` is the transcript.

---

## 4. Panel behaviour by kind and Perspective

Perspective values `engine | student | target`. Resolution: `variant = panel.variants[view] ?? panel.variants.engine ?? first`. Kinds declared `engine_only` (`values`, `plans`, `status`, and `alternatives` without continuations) show the engine content under every perspective with **no fallback line**. Other kinds, when the fallback was used, show one grey line: "No 10 kyu line for this card — showing the engine line." `counterfactual` under `engine` with no engine variant shows "Switch to Your level or Target level to see a human rollout" plus a button that sets the global perspective.

**In-card view control:** every card with ≥ 2 variants renders a segmented control at the top of its body (`role="radiogroup"`, buttons `role="radio"` `aria-checked`): `Engine | Your level (10 kyu) | Target (3 kyu)`. Its selected segment is the card's current view; it starts at the global perspective when the card opens and **resets when the card closes**; a global change re-syncs every card. When the card's view differs from the global one, the card header shows a chip "showing: Your level". No hidden state, no localStorage: the global perspective is `engine` on every load.

Defaults per kind (`first_color`, `after_played`) and content:

**played** — first_color = student, after_played = false. Sequence generated by the page: `[played] + allMoves[move_number .. move_number + follow_game_moves)` with real colours; the chips row is prefixed "The game continued: 1 = move 24, 2 = move 25 …"; view badge "Actual game moves 24–29". Text = `text` (legacy `explanation`). Under `student`/`target` a chip "Played by 30% of 10 kyu players" from `candidates[played].human_freq`. `follow_game_moves` default 0 for legacy lessons (their text never anticipated stones), 6 for schema 2. Missed opportunity: title "What you played (safe, but small)".

**engine** — first_color = student. `variants.engine.sequence = evidence.better_line.sequence`; numbers `score_after`, `visits`. `variants.student/target` = the **first `min(8, len)` plies of `counterfactual[view].sequence`** (`derived_from: "counterfactual"`; note: "How a 10 kyu typically continues from here — the first eight moves of the rollout below"). Text: `text.engine` explains the key moves by number; `text.student` (optional) one sentence on where the human line drifts and what it costs (numbers from `checkpoints[0..1]`). Missed opportunity: title "What was available: +6.0".

**allows** — first_color = opponent, after_played = true. `variants.engine` = `refutation.engine`. `variants.student/target` = `refutation[view]` with numbers `opp_reply_prob`, `opp_reply_delta`, `defence_delta`. Generator template (when `text[view]` absent, always shown as the numbers line): "The reply an 8 kyu most often plays here is F2 (41%). It costs you {|opp_reply_delta|} points {less|more} than KataGo's reply; answering it the way a 10 kyu usually does gives back {|defence_delta|} more." When `profiles.opponent == "engine"`: "Your opponent was a bot, so its reply is the engine's; the difference below is entirely in the defence." Missed opportunity: title "How the opponent closed the door", text must not call it a punishment (validator warns on "punish").

**human** — first_color = student, after_played = false. **Overlay at step 0 under every perspective:** `{E: best, loss 0}`, `{Y: human.student.top_move, loss}`, `{T: human.target.top_move, loss}`; coincident moves merge into one circle tagged "E/T" etc. The perspective only chooses which continuation plays after step 0: `student` → `human.student.continuation`, `target` → `human.target.continuation`, `engine` → none (overlay only). **Dedupe:** when `human[view].top_move == best`, the variant is omitted and the card says "Most 3 kyu players find D3 (60%) — the same as KataGo; see What KataGo prefers"; when `top_move == played`, "This is what you played (30% of 10 kyu players do) — see What happened". Summary regenerated: "10 kyu players: E2 (30%) · 3 kyu: D3 (60%)". Text: `text.engine` = the level comparison (legacy `level_framing`); `text.student/target` = why that move is natural at that level and what it costs.

**alternatives** — overlay at step 0: best ★, every candidate, the played move; quality colour and `−n`. Rows built by the generator from `evidence.candidates`: **a ★ row for the best move first** ("KataGo's choice — see What KataGo prefers"), then candidates by `loss` ascending, **the played move last** (red, its deep quality); the skill supplies prose per move in `rows` (missing prose → the row shows only numbers). Row = quality badge with text ("mistake · −4.1"), move, "−n pts", frequency chips "10k 12% · 3k 4%" when present, prose. Selecting a row (click, or ↑/↓ + Enter in the `role="listbox"`) places the stone in its quality colour and plays `pv` (engine) as the continuation. Perspective: `engine_only` unless the evidence has human continuations for that candidate (not planned in v5; the field is reserved).

**values** — "How big is a move here". Overlay: each candidate circle shows `worth_pts` (one glyph) with S/G tag when `cell ≥ 24` and board ≥ 13. Body, generator template, stated **once**: "The best move here is worth about 9.2 points — that is how much you would lose by passing. A typical move at this stage of the game is worth about 5.5, so this position is urgent: 7.0 of those points are in this local area (what your opponent's follow-up here would take)." Then rows "D3: 9.2 · C2: 6.0 · your A19: −2.9". When `worth_pts(played) < 0`: "Your move was 2.9 points worse than passing — it actively hurt." Endgame lessons add "about 4.6 points by the usual endgame counting (half the swing)" (`worth_pts / 2`). Sente rows (§6.5) only for candidates with `sente.class` non-null; others show "—" with tooltip "the local reply here is a punishment, not an answer". Caveat line: "Based on where KataGo's best reply lands." Summary: "best move worth 9.2 pts · urgent". `engine_only`.

**plans** — "Where the points go". Evidence: `ownership(played)` = root ownership of turn i+1 (deep, 1000 visits in two-pass); `ownership(best)` = `movesOwnership` of the best move at turn i (require `best_visits ≥ 50`, else the panel is omitted and `plans: null`). `delta(p) = sign · (own_best(p) − own_played(p))` in the **student's** perspective, computed by the report over all points; `points` = up to 24 with `|delta| ≥ 0.25`; `regions` = per 9-region grid total over *all* points, keep `|delta| ≥ 2`, at most 3, sorted by |delta|. Board: ▲ where delta > 0, ▼ where < 0. Rows: "Lower left: the better move (D3) is worth 5.2 more points for you here than A19." / "Upper side: A19 grabbed 1.8 points here, but lost more elsewhere." Store `movesOwnership` only for teaching turns (KataGo returns it for every moveInfo; unbounded storage would add 5–10 MB to the JSON dump). `engine_only`.

**tree** — §5.

**counterfactual** — first_color = student, sequence starts with the better move, 20–30 plies (`method: human_policy_argmax_1visit`, both sides from the asymmetric profile; the opponent from the engine top move when `profiles.opponent == "engine"`). Extras rendered by the generator: checkpoint table with **two rows** — "this line" from `checkpoints` and "the real game" from `allMoves[move_number − 1 + ply].score_after_num` (missing → "—") — and a final line "After 25 moves: this line you −2.0 · the real game you −9.5". Sparkline: inline `<svg viewBox="0 0 240 40">` with two polylines (this line solid, real game dashed) plotting **score in student perspective** (never winrate), current step marked with a dot, `<title>` text alternative. Winrate columns hidden when `decided`. Skill writes only the qualitative sentence(s): does the better move survive at your level, where the rollout gives it back. Note under the text: "Every move here is the single most likely move for a 10 kyu (student) and an 8 kyu (opponent) according to KataGo's human model, without search; the scores are KataGo's evaluations every five moves."

**status** — "Life and death here". Evidence:
```jsonc
"status": {
  "group": {"color": "B", "stones": ["D3","E3"], "anchor": "D3"},
  "verdict": "unsettled",                                // status_of(colour, mean root ownership over the group's stones) with the existing ±0.4 thresholds (teaching.rs)
  "to_move": "B",
  "attacker_first": {"sequence": ["C2","D2","B3"], "first_color": "W", "result": "dead",  "group_ownership": -0.82, "via_pass": true},
  "defender_first": {"sequence": ["D2","C2","E2"], "first_color": "B", "result": "alive", "group_ownership": 0.71, "via_pass": false},
  "swing_pts": 7.5,                                      // Σ over the local area of |own_defender_first − own_attacker_first|; local area = group stones ∪ points within Chebyshev 2 of them
  "killing_move": "C2", "saving_move": "D2",
  "search": {"visits": 300, "allowed": ["B2","C2","D2","E2","B3","F3","pass"]},
  "human_find": {"student": 0.35, "target": 0.71},        // raw humanPolicy of saving_move (or killing_move) at each profile, optional
  "puzzle_seed": { ... §7.3, optional ... }
}
```
Algorithm **[report dep]**: local area = group ∪ Chebyshev ≤ 2 (13x13/19x19) or ≤ 1 plus adjacent empty points (9x9); `allowed` = empty points of the local area **plus `pass` for both players** (so the search is not forced into self-destructive moves once the fight is settled). Two restricted searches at 300 visits with `allowMoves` for both players, `untilDepth` 6: one from the position (side to move plays first), one with a pass by the side to move appended (`via_pass: true`) so the other side plays first. `result` = `status_of` on the **restricted search's root ownership** over the group's stones (local verdict, per the brief). Sequence = the restricted PV truncated at 8 plies or the first pass. `swing_pts` from ownership as above — never from `scoreLead`. Panel body: two sub-buttons inside the card labelled with outcome ("If White attacks first → dead", "If Black defends first → alive"), default = the side to move; a line "Showing: if White attacks first → dead". Board: group ▲, allowed points hatched (`#24a` alpha 0.12), the chosen sequence numbered. Text template: "**Playing here first is worth about 7.5 points.** If White attacks first the group dies; if Black defends first it lives. KataGo read only the hatched points." Never "the group is worth". `verdict_text` from the skill (plain words) above the template. Under human perspectives an optional chip "a 10 kyu finds D2 35% of the time" from `human_find`. `engine_only`.

**generic** (unknown kind): title = `panel.title || kind`; plays inline `variants` like `engine`.

---

## 5. Variation tree explorer

Evidence (built by the report, never by the model):
```jsonc
"tree": {
  "to_move": "B", "depth": 2, "node_count": 12,
  "nodes": [
    {"move": "D3", "mover": "B", "score": -7.5, "winrate": 0.004, "visits": 812, "loss_for_mover": 0.0,
     "deeper_score": -7.3, "evaluated_by": "search", "human": {"student": 0.40, "target": 0.60},
     "children": [
       {"move": "C3", "mover": "W", "score": -7.4, "visits": 120, "loss_for_mover": 0.0, "evaluated_by": "search", "human": {...}, "children": []},
       {"move": "E2", "mover": "W", "score": -10.4, "visits": 31, "loss_for_mover": 3.0, "evaluated_by": "parent", "human": {...}, "children": []}
     ]},
    {"move": "A19", "mover": "B", "played": true, "loss_for_mover": 12.1, ...}
  ]
}
```
**Consistency rule [report dep]:** every sibling's `score` and `loss_for_mover` come from the **same parent search's moveInfos** (so there is exactly one `loss_for_mover == 0` child and no negative losses); a node's own search is used only for its *children* and stored as `deeper_score` ("deeper: you −7.3"). `evaluated_by: "parent"` nodes with `visits < 20` are `weak: true`. Limits: default **depth 2, ≤ 3 root children, ≤ 2 children each, ≤ 20 nodes**; depth 3 (≤ 40 nodes) only for the lesson with the largest loss. Cost: root children free (pass-2 moveInfos); one 200-visit search per expanded root child ≈ 2 s → **≈ 6 s per lesson** (depth 3: + 6 × 2 s = 18 s). Human frequencies: `humanPolicy` of each searched position under the asymmetric student/opponent profile comes back with the same query; the target-profile frequencies need one 1-visit query per searched position (0.1 s each, ≈ 0.5 s per lesson).

Rendering: `<ul class="tree" role="tree">`, node `<button role="treeitem">`: colour dot (●/○ — student-move nodes on a cream chip, opponent-move nodes on a grey chip; two-item legend), move, and **one** of ★ (best sibling) or a `−n` badge coloured by quality, with the owner in the visible text and tooltip: "White gives back 3.0" for an opponent node, "your mistake: −12.1" for a student node. Score (student perspective) is shown **only for the selected node** in the body ("After D3 C3: you −7.4 · deeper: −7.3"); visits and "from the parent search" only in tooltips; `weak` nodes show "?" in the tooltip text and a dotted border. Under `student`/`target` the frequency chip "40% at 10 kyu" appears on each node. Keyboard: roving tabindex; ↑/↓ siblings, → expand/first child, ← parent, Enter/Space select.

Board: selecting a node plays root→node (numbered from `to_move`) and overlays the node's children as ghost circles (quality colour, `−n`, alpha 0.7); clicking a ghost selects that child. Default path: `engine` → follow `loss_for_mover == 0`; `student`/`target` → the child with the highest `human[view]` (ties → lower loss). Summary: "engine: D3 C3 · likely at 10 kyu: D3 E2".

---

## 6. Numbers: display and thresholds

### 6.1 Formatting (JS `fmt`, mirrored in Python)
- `parseLead("W+0.3") → -0.3`, `parseLead("B+4.1") → 4.1`, `parseLead("0.0"|"jigo") → 0`; `parsePct("47.2%") → 0.472`. Both Python (parser adds `*_num` fields) and JS (for legacy strings).
- Score, Black number `s`: absolute "B+7.5"/"W+7.5"; student perspective "you −7.5"; header and all templates use the student form and add the absolute in a tooltip.
- Winrate: "Win chance: you 99.7% → 99.9%" (student perspective). When `decided` (evidence) or `winrate_before ∉ [0.05, 0.95]`, the header shows one grey chip "game already decided — judge by points" instead and every winrate column in tree/checkpoints is hidden.
- Loss: "12.1 pts worse than the best move"; quality badges always carry word + number ("big mistake · −12.1"); board markers carry a number or ★.
- Probabilities: `< 0.001 → "<0.1%"`, else `Math.round(p*100) + "%"`.
- Header strip: **at most four chips**: `big mistake · 12.1 pts worse`, `Score: you −7.5 → −14.7`, the winrate chip or the decided chip, `Played by 30% of 10 kyu · 2% of 3 kyu` (when human_freq exists). Missed opportunity: first chip `Missed: +6.0 pts available`. Tempo value, sharpness and urgency live in the `values` card's summary, not in the header.

### 6.2 Quality thresholds
Unchanged: < 0.5 best, < 1.5 good, < 3 inaccuracy, < 6 mistake, < 12 big mistake, else blunder. Colours: best `#2a7a2a`, good `#5cb85c`, inaccuracy `#b8860b`, mistake `#d2691e`, big mistake `#c0392b`, blunder `#8e1b1b`. Every colour carries its label text wherever it appears.

### 6.3 Sharpness **[report dep]**
`within_1pt` = number of pre-move candidates with `loss < 1.0` **counting only candidates with `visits ≥ 5% of root visits`** (`counted` = how many qualified). Labels: 1 → "sharp: roughly one good move"; 2–3 → "a few good moves"; ≥ 4 → "many equally good moves". Phrased "roughly" because 1 point is within the spread of a 500-visit search on 19x19. Findability from the raw policy: `policy_top = policy[best]`; `findability = policy_top ≥ 0.5 ? "obvious" : policy_top < 0.1 ? "hidden" : "findable"`; `policy_entropy` (nats over legal moves) stored for the report. UI: "sharp: roughly one good move · the move was hidden (KataGo's first instinct gave it 6%)".

### 6.4 Tempo value and urgency **[report dep]**
- `pass_value` = `score(best) − score(after an appended pass)` in the mover's points, 300 visits. Displayed as **"what the best move here is worth"** (tempo value), never as urgency.
- `phase_typical` = the median `pass_value` of the positions probed in the same phase of this game (cheap tier probes every teaching candidate, so ≥ 3 per phase are usual; fewer than 3 → the report falls back to the 19x19 defaults opening 14, middlegame 8, endgame 3, or 9x9 defaults 6 / 4 / 1.5, marked `phase_typical_source: "default"` and the UI says "a typical move at this stage is usually worth about").
- `urgency_local` = how much of the tempo lies in this local area: from the position with the student's pass appended, run 300 visits with `avoidMoves: [{player: opponent, moves: local(best), untilDepth: 1}]` (local = Chebyshev ≤ 3 of `best` on 13/19, ≤ 2 on 9x9) → `score_nonlocal`; `urgency_local = sign · (score_nonlocal − score_pass)`. Interpretation: if the opponent, forbidden to play locally, does almost as well, the area is not urgent.
- `urgency = urgency_local ≥ 0.75 · pass_value ? "urgent" : urgency_local ≤ 0.25 · pass_value ? "quiet" : "normal"`. **To verify empirically:** run the two probes on ~30 positions per phase from three games; check that positions the report marks "urgent" are mostly life-and-death / cutting themes and "quiet" mostly big-point moves; tune the 0.75 / 0.25 factors. Until verified the panel prints all three numbers (worth, typical, local) as in §4 values so the label is never the only information.

### 6.5 Per-candidate worth and sente/gote **[report dep]**
- `worth_pts_i = pass_value − loss_i` (no extra query; for the played move use the deep post-move `loss`). Presented as "points better than passing"; endgame lessons add the half-swing caption. Negative → "worse than passing".
- Sente/gote computed **only for candidates with `loss < 3`**. For candidate m: from the position with m appended, take the opponent's best reply `r` (search 300 visits); `local` = Chebyshev ≤ 3 (≤ 2 on 9x9) of m **or adjacent to m's group**; `tenuki_cost` = `score(best reply) − score(best non-local reply)` in the opponent's points, where the non-local reply comes from a 300-visit search with `avoidMoves: [{player: opponent, moves: local points, untilDepth: 1}]`. `class = local && tenuki_cost ≥ 4 ? "sente" : (!local || tenuki_cost ≤ 1.5) ? "gote" : "unclear"`. Thresholds sit outside 300-visit noise (~1–2 pts) rather than at the good/mistake boundaries. On **9x9 no S/G glyphs**; rows show "ignoring it would cost White 5.1" only. Rust: `QueryExtras` gains `avoid_moves: Option<(Color, Vec<String>, u32)>` (player, moves, untilDepth) — today it has only `allow_moves` for both players.

### 6.6 Missed opportunity **[report dep]**
`kind = "missed_opportunity"` when `loss ≥ 3` and, from `movesOwnership`, the best move takes something of the opponent's while the played move lost nothing of the student's own: `Σ_{opponent stones} sign·(own_best − own_played) ≥ 3` and `Σ_{student stones} sign·(own_played − own_best) < 1.5` (requires `best_visits ≥ 50`, else never). `gain_available = loss`. Panel set: `played` ("What you played (safe, but small)"), `engine` ("What was available: +6.0"), `allows` ("How the opponent closed the door", opponent first, engine reply after the played move), `counterfactual`, then the theme deep card. To verify: inspect the flagged moments in three games; the definition should catch "didn't capture / didn't kill / didn't invade" and not "over-defended".

### 6.7 Rank estimate **[report dep]**
Profiles `{rank_20k, rank_15k, rank_10k, rank_5k, rank_1k, rank_3d}`, one 1-visit query per profile with `analyzeTurns` = all positions where the student is to move (~0.1 s per turn; 100 turns × 6 ≈ 60 s). Per phase and overall: `LL_r = Σ log max(humanPolicy_r[played], 1e−4)`; estimate = the profile maximising LL, refined by a parabola through the three neighbouring `(rank_num, LL)` points (clamped to the range); `band` = all profiles with `LL ≥ LL_max − 2.0` (likelihood ratio e²) — to verify against the SGF rank field of the student's earlier games in `progress.rs`. Confidence from move count: < 40 rough, < 120 fair, else good. UI card: "Estimated from 142 of your moves: about 9 kyu (8–10 kyu) — opening 7 kyu, middle game 9 kyu, endgame 11 kyu. Estimated from how likely each of your moves is for players of each rank." When `|estimate − profiles.student| ≥ 3` stones add "The human lines in this lesson use the 10 kyu profile chosen at upload." **"Your level" labels always come from `profiles.student`**, because every human line was computed with it.

---

## 7. Puzzles

### 7.1 Schema additions (all optional)
```jsonc
Puzzle += {
  "acceptable_moves": [{"move": "C2", "loss_vs_best": 1.0, "explanation": "Also lives, but gives White the E2 hane."}],
  "continuation": ["E2","C2"],                        // opponent's usual reply after the correct move, opponent first (engine-style, skill's judgement)
  "wrong_moves": [{ "move": "F3", "quality": "blunder", "loss_vs_best": 10, "explanation": "...", "refutation": ["D2"],
                    "refutation_variants": {"student": {"sequence": ["D2"], "text": "Even a 10 kyu sees the atari."}, "target": {...}} }],
  "max_attempts": 3,
  "source": {"report_move": 23, "transform": "rot90"}    // §7.3 only
}
```
**No `prob`, no `human_freq`, no frequency chips in puzzles** (validator PROBLEM if present): puzzles are designed without an engine. Skill-authored `refutation_variants` are rendered under the label "A typical reply at your level (teacher's judgement, not measured)". `SKILL.md`: write them only when the lesson's realistic refutation shows the *same reason* (e.g. "a 10 kyu still finds the atari, but not the follow-up") and never quote a percentage.

### 7.2 Interaction and feedback
Board click as today (with the DPR fix); the puzzle board gets the same stepper; coordinate `<input>` + Go button for keyboard users. Buttons: **Hint**, **Show answer** (renamed from "Show evaluations": ★, overlay of listed moves with `−n`, comparison list), **Reset**. Feedback card `role="status" aria-live="polite"`:
- **Correct**: green border, "✓ Correct — D2 (best)"; explanation; if `continuation`, "How the opponent usually answers" and the stepper plays it (opponent first).
- **Acceptable**: amber-green, "~ Good — C2 works, but D2 is best (−1.0 pt)"; explanation; [Try again] [Show answer].
- **Listed wrong move**: red, "✗ F3 — blunder, about 10 points worse than the best move"; "Punishment" (engine-style) or, when the current perspective has a `refutation_variants` entry, "A typical reply at your level (teacher's judgement)" — with the in-card segmented control; sequence numbered, stepper active; explanation; "Attempt 2 of 3". At `max_attempts`: "Out of attempts — see the answer", Show answer highlighted.
- **Unlisted wrong move**: red, "✗ G5 — not the best move", `generic_wrong_explanation`.
Reset clears state, attempts, stepper. Perspective changes re-render the current feedback without resetting attempts. Legend: "■ opponent's last move · ● green: correct · ● coloured: tempting wrong moves (colour and −n show how bad) · numbered stones: the punishment".

### 7.3 Engine-verified puzzle seeds **[report dep, optional in v5.0]**
For each `status` block the report may add `puzzle_seed`: `{black_stones, white_stones, to_move, correct_moves: [saving_move|killing_move], wrong_moves: [{move, loss_vs_best, refutation}] (from the restricted search's moveInfos and their PVs, ≤ 3), board_size}` — the local area (group ∪ Chebyshev 3), re-based to a 9x9 or 13x13 board when it fits. The skill uses it by writing `"source": {"report_move": 23, "transform": "rot90|rot180|rot270|mirror|swap_colors|none"}` plus prose (`title`, `hint`, `explanation`, wrong-move explanations); **the generator applies the transform** and fills the stones and moves, so the model never rewrites coordinates. Such puzzles carry the badge "verified by KataGo (local reading, 300 visits)" and their `refutation` sequences are engine lines. The validator checks that a seeded puzzle has no hand-written stones.

---

## 8. Generator implementation (`generate_lesson.py`)

### 8.1 Python side
1. `normalize_lesson(lesson, parsed) -> dict`: resolve `player_color`; find `cand = candidates[move_number]` and `ev = cand.evidence`; apply §9 legacy mapping when `lesson_schema < 2` or `ev is None`; build panels from evidence in canonical order (`played` always; `engine` if `better_line`; `allows` if `refutation`; `human` if profiles and any `human[*]`; `alternatives` if `candidates`; deep panels by theme rule, `counterfactual` if present; `status` only if `ev.status`; `values` if `tempo`; `plans` if `plans`; `tree` if `tree`); apply `use: false`; attach `text`/`summary`/`note`/`rows`; enforce the 6-card cap (collapse the rest under "More detail"); truncate limits (sequence ≤ 40, tree ≤ 40 nodes, overlay per §3.3, ownership points ≤ 24); dedupe `human` variants (§4).
   Summary templates (student perspective, with a verb): `played` → "big mistake: 12.1 pts worse"; `engine` → "D3 keeps you at −7.5"; `allows` → "punished: you end at −14.7 (from −7.5)"; `human` → "10 kyu players: E2 30% · 3 kyu: D3 60%"; `alternatives` → "5 moves compared"; `values` → "best move worth 9.2 pts · urgent"; `plans` → "lower left: 5.2 pts hinge on this move"; `tree` → "engine: D3 C3 · likely at 10 kyu: D3 E2"; `counterfactual` → "after 25 moves: you −2.0 (real game −9.5)"; `status` → "unsettled: whoever plays here first decides".
2. `build_header_chips(ev, lesson)` (§6.1, ≤ 4 chips, `<span class="chip"><span class="chip-k">Score</span> <span class="chip-v">you −7.5 → −14.7</span></span>`).
3. `build_panel_cards(i, lesson)`: accordion headers `<h3><button id="ph-i-k" aria-expanded aria-controls="pb-i-k" data-kind>` and bodies `<div id="pb-i-k" class="panel-body" role="region" hidden>` containing: the segmented control (when ≥ 2 variants), a `<div class="variant-text">` holding the engine text server-side (readable without JS), and one `<template data-variant="student|target">` per other variant with its pre-rendered HTML (`escape_html`, `format_paragraphs`); kind-specific static parts (alternatives `<ol role="listbox">`, values table, plans rows, tree `<ul>`, status sub-buttons, checkpoint table + sparkline `<svg>`). **Variant HTML lives only in the DOM**; `lessonData` carries sequences, overlays and numbers.
4. `build_perspective_bar(profiles)`, `build_rank_card(rank_estimate, text)`, `build_puzzle_section` (stepper, rename, seed transform).
5. `escape_html` (keeps `**bold**`), `format_paragraphs`, new `escape_attr`.
6. Emit `const setupStones`, `const allMoves` (with `*_num`), `const profiles`, `const lessonData`, `const puzzleData`, `const goodMoveData`.

### 8.2 JS side (single inline `<script>`, ES2017)
- `GoBoard`, `gtpToXY`, `xyToGtp` unchanged; `replayMoves` unchanged but always preceded by `placeStones(setupStones)`.
- `Renderer`: `LOGICAL = 720`, DPR transform, `computeLayout(LOGICAL)`, `toXY(scale = LOGICAL / rect.width)`, marker types `circle | triangle | square | label | shade | hatch | ghost`, label halo, grey game numbers, glyph-density rule, `setAria(text)`.
- `qualityFromLoss/Color/Label` unchanged thresholds; label always "name · −n".
- `Perspective` store `{value:'engine', set, subscribe}`; bar buttons `role="radio"`; no persistence.
- `SequenceView(canvas, baseBoard, oppMarker, studentColor, size)`: `steps`, `stepIndex`, `overlay`, `groupMarks`, `hatch`; `render()` clones the base, replays to `stepIndex` tracking survivors, draws overlay when `stepIndex ≤ playedStep`, updates chips, step label (`aria-live`), disabled states.
- `LessonView(i, data)`: accordion (single open; closing resets that card's view to the global perspective), per-card `view`, kind handlers (`onAlternativeSelect`, `onTreeSelect`, `onStatusSide`, `onHumanTag`), board click for overlays, template swapping, legend/badge updates. No `history` manipulation.
- `PuzzleView(i, data)`: as today + stepper, states §7.2, perspective-aware variants, seed transform applied client-side too (the Python already baked it; JS only reads).
- `esc(s)` for any string composed at runtime.
- `init` on `DOMContentLoaded`; adds class `js` to `<html>` first.

### 8.3 Keyboard
- Perspective bar and in-card controls: radio-group semantics (←/→ change value, each button focusable).
- Accordion: ↑/↓ between headers of the same lesson, Home/End, Enter/Space opens (and plays); focus stays on the header.
- Stepping: ←/→ step and Shift+←/→ jump, while focus is inside a lesson or puzzle `<section>` **except** when `event.target` is `input, textarea, select` or is inside `[role=radiogroup], [role=tree], [role=listbox]` (they own the arrow keys; the stepper buttons are Tab-reachable there). `Escape` → step 0. No single-letter shortcuts.
- Puzzle: canvas `tabindex="0"` with a focus ring; the coordinate `<input>` + Go makes puzzles keyboard-complete.

### 8.4 CSS notes
Palette as today (`#f5f0e8`, `#dcb35c`). Tokens `--good #2a7a2a`, `--bad #c0392b`, `--info #24a`, `--human #6a4c93`; focus ring `outline: 3px solid #24a; outline-offset: 2px`. Card header: full-width button, bold kind title, grey ellipsised summary, right chips; open header gets a 4-px left border in the panel colour (played red, engine green, allows orange `#d2691e`, human purple, others `#5a7a9a`). Segmented control: pill group, selected segment filled. Reduced motion respected (only `transition: background .15s`). `@media print`: all bodies expanded, steppers hidden, engine text shown. `html:not(.js) .panel-body { display:block }` for no-JS.

### 8.5 Size/perf
Per lesson data ≈ 6–12 KB (counterfactual 2 × 25 plies + checkpoints, tree ≤ 20 nodes, overlays); a game with 3 lessons and 3 puzzles < 80 KB of data, JS ≈ 45 KB. One canvas redraw per step (< 5 ms on 19x19). No timers. Lesson JSON written by the skill drops to ≈ 3–5 KB per lesson (prose only).

---

## 9. Migration: legacy lesson JSON and inline panels

Applied by `normalize_lesson` when `lesson_schema < 2` or `evidence` is null for that move (legacy fields fill only kinds the evidence did not).

| Legacy field(s) | Panel / place |
|---|---|
| `explanation` | `played.text.engine`; `follow_game_moves` = 0 |
| `preferred_move`, `better_line`, `variation_explanation` | `engine.variants.engine.sequence = better_line || [preferred_move]`, `.text` |
| `refutation`, `refutation_explanation` | `allows.variants.engine` (opponent first, `after_played`) |
| `alternatives[]` (with `loss_vs_best`, `quality`, `explanation`) | `alternatives` rows + overlay (best ★ prepended, played appended) |
| `level_framing` | `human.text.engine`; overlay E only unless `move_details[n].human_top_move`/`target_top_move` exist (then Y/T from those, losses from `candidates`) |
| `winrate_before/after`, `score_before/after` strings; else `move_details[n]` | header chips via `parseLead`/`parsePct` |
| `point_loss`, `played_quality` | header chip + `played.summary` |
| `story`, `principle`, `theme`, `concept_label` | unchanged |
| Puzzle `wrong_moves[].refutation` | `refutation_variants.engine.sequence` |

Inline panel data (`data: "inline"`) uses the same variant shape as the draft: `variants.{engine,student,target} = {sequence, first_color, after_played, overlay, text, numbers, note}`; numbers may only be report strings or generator-derived; it is intended for legacy and hand-made demonstrations and is a validator PROBLEM for evidence-backed kinds when evidence exists.

Behavioural equivalents: "Position" = step 0 / Escape; "Show played move" = `played`; "Show better move" = `engine` step 1; "Show all candidates" = `alternatives` step 0; "What it allows" = `allows`; "Better line" = `engine` last step; alternative buttons = rows. `tests/sample_lesson.json` (v1) with `tests/sample_report_format4.md` must generate without warnings and show five cards.

---

## 10. Validator changes (`validate_lesson.py`)

Keep every existing check; `replay` and the new `play_sequence(board, seq, first_color, size)` start from `game_info.setup_*`. Add:
1. Report format ≥ 5 and no evidence for a chosen candidate → warning ("report has no evidence for move N; legacy rendering").
2. Schema-2 lesson: any numeric `score*`, `winrate*`, `loss*`, `prob`, `human_freq` written by the skill outside `data: "inline"` panels → PROBLEM ("numbers come from the report; remove"). Inline data for `tree|plans|values|status|counterfactual|human|allows|engine` while evidence exists → PROBLEM (drift).
3. Inline/legacy sequences: legality from the base position (after the played move when `after_played`), overlay points empty, `first_color` ∈ {B,W}, length ≤ 40 (warning > 12 for non-counterfactual), with `lesson 2 (move 31): panel allows/student ply 3 D4 is occupied`.
4. Text ↔ board: regex `\b(?:after|move|stone|with)s?\s+(\d{1,2})\b` over each `text[view]` — a number above that variant's sequence length → PROBLEM; `text[view]` for a view with no line → warning; a line with no text (inline) → warning.
5. Panels per lesson: fewer than 3 → warning; more than 6 → warning (collapsed). `summary` > 90 chars → warning; a `summary` containing a bare `B+`/`W+` → PROBLEM (must be student perspective).
6. `human` panel text mentioning a move that is not `best`, `played`, `human.student.top_move` or `human.target.top_move` → warning.
7. Missed-opportunity lesson whose `allows` text contains "punish" → warning.
8. `profiles`: `text.student/target` used but `game_info.profiles` lacks that key → PROBLEM.
9. Puzzles: `prob`/`human_freq` anywhere → PROBLEM; `refutation_variants.*.sequence` and `continuation` legal; `acceptable_moves` distinct from correct and wrong; seeded puzzles (`source.report_move`) must not carry hand-written stones and the seed must exist.
10. Unknown `kind` → warning. Exit code unchanged (1 on problems).

---

## 11. What `SKILL.md` must instruct

Preamble for step 5 "Panels": "Write `panels` for each lesson. Each panel is one view the student clicks. You write **prose only**: `summary` (≤ 90 characters, in the student's perspective, with a verb), `text` per view (`engine`, and `student`/`target` only when `evidence` has that line), and optionally `note`. Every sequence, score, visit count, probability and coordinate list comes from the report's evidence block and is placed by the generator — never copy or retype them. Refer to stones by their numbers in the line the panel plays (the generator checks that the numbers exist). Set `lesson_schema: 2`."

- **played**: "The tactical why (what the move does, what it ignores, where the points went — `plans.regions`); do not narrate the continuation: the page plays the real game moves in grey."
- **engine**: "Explain KataGo's line by stone number (`better_line`). If `counterfactual.student` exists, one sentence on where a 10 kyu drifts from it and what `checkpoints` say it costs."
- **allows**: "Explain the engine punishment by number. For `student`/`target`, use `opp_reply_prob`, `opp_reply_delta`, `defence_delta`: say what the realistic reply is, whether it is milder or harsher than KataGo's, and how much of the damage is the student's own defence. For a bot opponent say the reply is the engine's."
- **human**: "`text.engine` = the level comparison from `human.*.top_move`/`prob` and `candidates[].human_freq`. When the 10 kyu or 3 kyu move equals KataGo's or the played move, the generator says so — do not write a variant text for it."
- **alternatives**: "`rows`: one or two sentences per candidate keyed by move, for 2–4 of `evidence.candidates`. Do not list the best or the played move (rows are added automatically)."
- **values**: "One sentence on what 'urgent'/'quiet' means in this position, using `tempo.pass_value`, `phase_typical`, `urgency_local` in words. Do not repeat the numbers three ways."
- **plans**: "One sentence per region in `plans.regions` (≤ 3), in the student's perspective."
- **tree**: "`summary` only if you want to override the generated one; `text.engine`: one sentence on what the tree teaches."
- **counterfactual**: "Does the better move survive at your level? Where does the rollout give it back? The generator prints the score comparison with the real game — do not compute it."
- **status**: "`verdict_text` in plain words ('lives if Black plays first, dies if White does'); never say 'the group is worth N' — the number is the value of playing first."
- **Puzzles**: "Engine-style punishments in `refutation`; `refutation_variants` only when justified by the lesson's realistic refutation and never with a percentage; `acceptable_moves`; `continuation`; prefer a `puzzle_seed` with a `transform` when the report offers one."
- Step 7: "Run the validator; fix every PROBLEM; read every warning about panels."

`references/html-build-guide.md`: replace "Lesson interaction / Sequence playback / Alternatives" with §1–§5 in the skill's voice; document the evidence block fields the skill may *quote in prose*; keep the field reference; add §9 under "Older lesson JSON".

---

## 12. Edge cases (generator behaviour)

- No `profiles` → no perspective bar, no in-card controls; human variants ignored except when they are the only variant (badge "human line").
- Only `profiles.student` → bar shows Engine | Your level.
- Student is White → all "you" conversions flip; default `first_color` follows `player_color`; the red circle is a white stone.
- Handicap game → setup stones placed on every board; move 1 has no blue square; `first_to_move` respected by the validator's colour check.
- `played_move == "pass"` → played step shows chip "pass", no red circle; `allows` starts from the base board.
- `follow_game_moves` past the end of the game → truncated.
- Sequence > 40 → truncated with chip "…(truncated)".
- Counterfactual with captures → struck chips; transcript "(captured at 17)". Plies > 12 dimmed numbers.
- Real-game continuation with captures → same, grey.
- Tree node whose path is illegal (inline data only) → disabled with title.
- Two lessons at the same move → independent views (ids carry the lesson index).
- `decided` → winrate chip replaced, winrate columns hidden once per lesson with the sentence "the game was already decided here — judge by points".
- Evidence present but lesson has no `text` for a card → the card renders with the generator template only (numbers line) and a stderr warning.
- JS disabled → static page: header chips, story, engine texts of all panels expanded, no boards.

---

## 13. Verification plan

1. `python3 scripts/generate_lesson.py tests/parsed_v4.json tests/sample_lesson.json out.html` on the v1 sample (parsed from `tests/sample_report_format4.md`): five cards, legacy behaviour reproduced, no console errors.
2. New `tests/sample_report_format5.md` (fake-engine run with `evidence` blocks; **one handicap game, student White**) + `tests/sample_lesson_v2.json` exercising every kind, both perspectives, a 25-ply counterfactual with a capture, a depth-3 tree, a status card, a seeded puzzle; validator 0 problems; page checked at 1280 / 768 / 390 px (sticky behaviour per §1, no horizontal scroll).
3. Keyboard run-through: Tab to the bar, → to switch, Tab to lesson 1 headers, ↓ to "What it allows", Enter, ←/→ step, Escape, Tab into the in-card control, → to Engine (header chip "showing: Engine" appears), close card (chip gone, view reset), switch global perspective.
4. VoiceOver smoke: headers announce expanded/collapsed; step label announces once per step; canvas label static per card.
5. Retina click test: click every intersection of a 9x9 puzzle at dpr 2 and confirm `toXY` returns the clicked point.
6. Fault injection: occupied inline point, unknown kind, a `text.student` saying "after 7" on a 5-ply line, a puzzle with `prob` → red chip / generic card / validator PROBLEMs.
7. Fake-engine unit tests **[report dep]**: human-move selection with White to move picks the move best for White; tree siblings have exactly one zero loss; `status.swing_pts` uses ownership; `worth_pts(best) == pass_value`; `missed_opportunity` fires on a synthetic "didn't capture" position and not on an over-defence.

---

## 14. Engine-side algorithms and cost **[report dep]**

**Human move choice.** `humanPolicy` (array indexed by `policy_index(move)`, pass last) at profile P for the position; `toMove` from `rootInfo.currentPlayer`; `u_mover = toMove == 'B' ? utility : −utility` (KataGo reports `utility` from Black's perspective under `reportAnalysisWinratesAs=BLACK`).
- *Most common move* (quoted with its probability; ply 1 of realistic refutations, `human.*.top_move`, `human_find`): `argmax humanPolicy` over legal moves (1-visit query, 0.1 s).
- *Most likely line* (plies 2+ of realistic refutations and human continuations): search at 100 visits with the human profile set; weight `w(m) = humanPolicy[m] · exp(u_mover(m) / 0.5)` over `moveInfos` with `visits ≥ 5`; if the raw argmax is not among them, append it as a forced move, analyse the next turn once (≈ 1 s) and include it; pick `argmax w`. Deterministic. `selection: "most_likely"` in the evidence; the UI says "the most likely reply", never "the reply".
- *Rollout* (counterfactual, 20–30 plies): raw argmax at 1 visit per ply for both sides (`method: human_policy_argmax_1visit`), checkpoints every 5 plies at 200 visits, final at 300. Reason: utility-weighted choice needs a search per ply (≈ 1 s), i.e. 50–60 s per lesson for two variants; pure policy costs 2.5 s per variant and is honestly labelled.
- Profile per query: asymmetric `rank_{BR}_{WR}` with Black's rank first (`student == B ? rank_{student}_{opponent} : rank_{opponent}_{student}`); when `profiles.opponent == "engine"` use `rank_{student}` and take the opponent's moves from the engine top move (`order == 0`) at 100 visits.
- Bot opponents: KaTrain names `AI (Default)`, `AI (KataGo)`, `AI (Jigo)` → `engine`. Other `AI (...)` bots → `engine` too (a human rank would be a guess), with the header note "your opponent was a weakened bot; the engine's replies may be harsher than what it would have played". The SGF rank field, when present for a human opponent, sets the opponent profile.

**Tiered cost per game (this Mac, GPU shared with KaTrain; ~5 s per 500 visits, ~1 s per 100, ~0.1 s per 1-visit query; treat as a floor):**
- *Cheap tier, every teaching candidate (≤ 8):* pass probe 300 visits ≈ 3 s; sharpness, candidate frequencies, `plans` (from `includeMovesOwnership` on the pass-2 query, stored only for teaching turns), `worth_pts` free. ≈ 25 s.
- *Heavy tier, ≤ 4 lessons (top 3 by loss + the top missed opportunity):* realistic refutation 2 variants × (1 raw argmax 0.1 s + 5 plies × 1 s + 300-visit eval after ply 1 for `opp_reply_delta` 3 s + final 500-visit eval 5 s) ≈ 28 s; human continuations 0–2 × (4 × 1 s + 5 s) ≈ 0–18 s (often deduped); tree depth 2 ≈ 6 s (+12 s for the top lesson); counterfactual 2 × (25 × 0.1 + 5 × 2 + 3) ≈ 32 s; urgency probe 3 s; sente probes ≤ 3 × (3 + 3) s ≈ 18 s; status 2 × 3 s ≈ 6 s (life-and-death themes or status_changes only). ≈ 95–125 s per lesson → **≈ 7–8 min for 4 lessons**.
- *Rank estimation:* 6 profiles × ~100 student turns × 0.1 s ≈ 60 s.
- **Budget: ≤ 10 min per game on top of today's analysis**, issued with `priority` above any queued full-game pass. Degrade order when the budget is exceeded (the report records `timing_s` and which items were skipped): depth-3 tree → human continuations → target-level counterfactual → sente probes for candidates ranked 3+ → the 4th lesson's heavy tier.

## 15. Decisions on the previously open questions

1. Bot opponent → `profile: "engine"`, note for weakened bots (§14). 2. No rollout from the played move — the real game *is* that rollout; the realistic refutation is a separate short line. 3. Argmax, deterministic, `selection: "most_likely"`. 4. Tree depth 2 default, 3 for the top lesson. 5. 9x9: no S/G glyphs, Chebyshev 2 for "local". 6. Puzzles: no human probabilities; engine-verified seeds optional. 7. No per-lesson perspective control; the in-card control covers long pages. 8. Dark mode out of scope; print rule kept. 9. "Your level" = upload profile; estimate only in the Overview card. 10. Default perspective on load = Engine (the student should see the best move before the human line; the bar is one click away and sticky). 11. Urgency measured locally (§6.4), tempo value named honestly. 12. Status swing from ownership, never scoreLead.