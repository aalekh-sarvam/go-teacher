# Orchestration: run the lesson as a team

Read this when the platform can run sub-agents. The stages below are the same work SKILL.md
describes; only the split differs. Everything an agent needs is in its **slice file**. No agent
reads parsed.json, brief.json or the Markdown report. No agent invents a field that is not in
its slice. If the platform cannot run sub-agents, use the single-agent order at the end.

Vocabulary used in every template:

- **Slice** = the JSON file written by `scripts/slice_for_agent.py` for that agent.
- **Part** = the JSON file the agent returns. Every part has a top-level `"part"` key whose value
  is one of `overview`, `lesson`, `praise`, `concepts`.
- **Colour tokens** = strings like `"B D7"`, `"W E3"`, `"W pass"` from any `*_with_colours` list.
  Narrated lines are built only by copying these tokens in order.

---

## Stage 0 — sequential, orchestrator only

Do these in order. Do not start Stage 1 before step 6 is written down.

1. **Parse.**
   ```
   python scripts/parse_review.py report.md parsed.json --brief-output brief.json --facts-output facts.json
   ```
   If the parser refuses the format or evidence version, stop and tell the user the skill and
   go_teacher are out of step.
2. **Identify the student.** Read `game_info.student` and `game_info.student_reason`. Write the
   sentence "The student is Black/White because <reason>". Every later step uses that side.
3. **Read `student_profile`.** Copy `student_profile.estimate.wording` and
   `student_profile.caveat`. Note `working_rank.rank_label` (fall back to `estimate.rank_label`,
   then to `profiles_used.student` with `student_source`). Look up the **band** for that label in
   `references/level-ladder.md` (table 1). Write: "Working rank <label> → band <band>".
4. **Find the weakest phase.** In `arc.phases`, the phase with the highest student mean loss.
   Write its name.
5. **Choose 2–3 moments** from `teaching.candidates` with the level-ladder selection rules
   (level-ladder.md, section "Selection rules"), in this order:
   a. keep candidates whose concept (ladder table 1, by `theme`) is in the band or one band above;
   b. prefer candidates in the weakest phase, then by `point_loss`;
   c. distinct `theme` values — never two lessons with the same theme;
   d. prefer non-empty `refutation_with_colours`;
   e. a candidate two bands above the rank is not a lesson; note it as one "later" sentence
      for the overview agent;
   f. when `evaluation_coverage.status` is present, prefer `complete` candidates for the
      primary lessons; a `complete` candidate whose `investigation_coverage.status` is
      `not_selected` is still a full lesson (fewer panels). Absent fields = unknown coverage.
   Write the list: `move_number`, `theme`, band, phase, coverage (`complete` / `incomplete` /
   `disabled` / `unknown`, investigations `available` or not).
6. **Research plan (one paragraph).** For each chosen moment: the pattern name to search
   (from `theme` and the ladder's concept row), the Go Magic resource from the ladder, the
   Sensei's page name, and the tsumego tier. Also name the weakest phase for the overview
   agent's phase search. This paragraph is pasted into every Stage 1 prompt.
7. **Slice.**
   ```
   python scripts/slice_for_agent.py parsed.json --moment 19 --facts facts.json -o slice_19.json
   python scripts/slice_for_agent.py parsed.json --moment 29 --facts facts.json -o slice_29.json
   python scripts/slice_for_agent.py parsed.json --role overview --moments 19 29 -o slice_overview.json
   python scripts/slice_for_agent.py parsed.json --role praise -o slice_praise.json
   python scripts/slice_for_agent.py parsed.json --role concepts --moments 19 29 -o slice_concepts.json
   ```
   Use the actual chosen move numbers.

---

## Stage 1 — parallel, one agent per part

Dispatch all agents at once. Each receives: its slice path, the research plan paragraph, the
student sentence from step 2, the band from step 3, and one template below. Agents return
one JSON file each. Agents never write the overview, never read another agent's slice, and
never talk to the user.

### Template A — moment agent (one per chosen move)

```
You are writing ONE lesson for a Go student. Student side: <B/W>. Level: <wording sentence>.
Band: <band>. Register rules: references/level-ladder.md section "Prose register".
Your only board input is <slice_N.json>. Research plan: <paragraph>.

The slice contains, for move N: the teaching candidate (played_move, preferred_move, theme,
chain_before, chain_after, status_changes, captured_at, hints, refutation_with_colours,
better_line_with_colours, evidence.sequences[] with moves_with_colours, local_reading, tree,
difficulty, human/target policy, evaluation_coverage, investigation_coverage), move_details[N]
.candidates[] (loss_vs_best, visits, pv_with_colours), facts for this move (policy, coordinates,
groups, local_trials, rollouts, sequences, coverage), student_profile and arc.phases.

Coverage: facts.coverage.text is the only sentence you may base "verified" on — write
"verified" / "deep evaluation" only when evaluation_coverage.status is "complete"; with
"incomplete" or "unknown" (or no coverage fields) use neither word. If
investigation_coverage.status is not "available"/"partial", or evidence.unavailable has
teaching_investigations or the legacy deeper_search key, there are NO human, local or what-if
sequences for this move: omit panels.local, panels.what_if and sequence_explanations and write
no "human reply". Those keys say nothing about deep evaluation; the lesson still stands on
refutation_with_colours, better_line_with_colours, policies, chain and move_details.

Do, in this order:
1. Research the pattern named in the plan: Go Magic resource first (only URLs from the verified
   library in references/game-arc-commentary.md), then the Sensei's Library page, then one
   external puzzle example from the tier named in the plan (references/level-ladder.md table 3;
   parse it with scripts/parse_tasuki_tex.py, scripts/parse_sgf_problem.py or
   scripts/parse_ogs_puzzle.py; verify with scripts/solve_tsumego.py or the engine checker as
   references/grounding-and-practice.md describes). Research has no search or time cap.
2. Write the lesson prose. Every narrated line copies colour tokens in order, e.g.
   "White plays W F3, Black answers B D7, then W H8". Point names count from the corner
   using the coordinates in facts.coordinates (B3 on 9x9 = 2-3 point of the lower-left corner).
3. Write fact_checks for every number, colour, defender, location and status you assert.
4. Build ONE puzzle from the researched external example, transformed and verified.

Forbidden:
- any coordinate, capture, atari, liberty count, score or probability not present in the slice;
- alternating colours yourself; bare coordinate lists in narrated lines;
- the fields alternatives, played_quality, concept, refutation, better_line, point_loss,
  played_move, preferred_move, player_color inside the lesson object;
- praising any move; writing about the whole game; a second puzzle; a puzzle from the game
  position or a symmetry/colour swap of it; a puzzle whose source you did not open;
- calling the move "verified" or "deeply evaluated" unless evaluation_coverage.status is
  "complete"; inventing a human reply, restricted reading or what-if line that is not in
  candidate.evidence.sequences;
- engine jargon below 10k (see the register rules); Japanese terms without an inline gloss.

Return exactly this JSON (fact_checks text_path values are relative to THIS lesson as
/lessons/0/...; the merge renumbers them):
{
  "part": "lesson",
  "lesson": {
    "move_number": N,
    "title": "<imperative, one line>",
    "concept_label": "<English concept name>",
    "theme": "<copied verbatim from the candidate>",
    "story": "<2-4 sentences from chain_before/chain_after/status_changes with move numbers>",
    "principle": "<one habit sentence>",
    "level_framing": "<1-2 sentences from facts.policy: student and target probabilities bound to the played and preferred moves>",
    "panels": {
      "position": "<what to notice before moving>",
      "played": "<walk refutation_with_colours / the played sequence tokens; what it allows>",
      "best": "<walk better_line_with_colours / the best sequence tokens; why it works>",
      "alternatives": "<compare move_details candidates by loss_vs_best; say 'barely searched' for 1-visit moves>",
      "local": "<optional: restricted reading, defender named, prediction not proof>",
      "what_if": "<optional: what the two sampled futures illustrate; include later choices caveat>"
    },
    "sequence_explanations": {"<sequence id>": "<optional prose for that sampled line>"}
  },
  "fact_checks": [
    {"text_path": "/lessons/0/panels/played", "quote": "<exact substring>", "fact": "/lessons/N/<path in facts>", "expected": <value>}
  ],
  "puzzles": [ <one puzzle object per references/grounding-and-practice.md, with lesson_move_number N> ]
}
Omit optional panels you cannot ground. Do not add keys.
```

### Template B — overview agent

```
You are writing the game overview for a Go lesson. Student side: <B/W>. Level: <wording>.
Band: <band>. Register rules: references/level-ladder.md section "Prose register".
Your only input is <slice_overview.json>: game_info, student_profile, summary.accuracy,
summary.timing, arc.phases, arc.status_changes, moves[] (number, color, move, point_loss,
score_black, stones_captured), context_moves, facts.current, facts.history, and the chosen
moments (move_number, theme) with the "later" notes. Research plan: <paragraph>.

Do, in this order:
1. One phase search per phase the student struggled in (references/game-arc-commentary.md,
   "Sequential web searches", item 1). No pattern-name or puzzle research.
2. Write overall_feedback (2-3 short paragraphs): which side the student is; the level
   sentence quoted with its caveat as "plays like ..."; main strength; main weakness; the
   weakest phase. Use summary.accuracy and arc.phases numbers only.
3. Write game_arc: intro plus one entry per phase in arc.phases: phase, move_range, narrative
   (1-4 sentences), optional turning_points (move-tagged), optional anchor_move.
   A capture may be stated only when moves[].stones_captured > 0 for that move or
   arc.status_changes lists it; state the stone count.
4. Write progress.summary from facts.current and facts.history.prior_games only; one-game
   language if prior_games is empty. Do not write rows.
5. Write fact_checks for every statistic you quote.

Forbidden: naming a lesson's takeaway (the lessons do that); praising moves; inventing
captures, atari, ladders or "the game was decided"; comparing the student's clock with the
bot's; any number not in the slice; move-by-move narration.

Return exactly:
{
  "part": "overview",
  "overall_feedback": "<text>",
  "game_arc": {"intro": "<text>", "phases": [{"phase": "Opening", "move_range": "1-16", "narrative": "<text>", "turning_points": ["<Move N: note>"], "anchor_move": 16}]},
  "progress": {"summary": "<text>"},
  "overview_anchor_move": <int or omit>,
  "fact_checks": [{"text_path": "/overall_feedback", "quote": "<exact substring>", "fact": "/current/accuracy/mean_loss", "expected": <value>}]
}
```

### Template C — praise agent

```
You are writing the "moves played well" cards. Student side: <B/W>. Level: <wording>.
Band: <band>. Your only input is <slice_praise.json>: teaching.praise[] (move_number, move,
player, kind_label, note, stones_captured, gap, second, human_prob, rating, rating_source)
and moves[] (number, color, move, point_loss, stones_captured).

Do, in this order:
1. Take entries from teaching.praise in file order, up to 4. If fewer than 3 exist, add
   student moves from moves[] with stones_captured > 0, largest first, until you have 3
   (or run out).
2. For each praise entry write explanation (2-3 sentences):
   - what it achieved: copy `note` verbatim as the first sentence, then say the stone count
     when stones_captured > 0 and the gap when gap >= 1 ("the next best searched move was
     <gap> points worse");
   - why it was not obvious: only if human_prob is present and below 0.25 — "players at your
     level choose it about <human_prob*100>% of the time";
   - one habit sentence.
   For a moves[]-only entry, the explanation says only "captured <n> stones" plus one habit
   sentence.
3. Copy `rating` and `rating_source` verbatim when both are present; omit both otherwise.

Forbidden: any move not in teaching.praise or in moves[] with stones_captured > 0; any
kyu/dan label you compose yourself; "the engine praised this" for a move not in
teaching.praise; changing `note`; rounding gap or stones; praising the opponent's moves.

Return exactly:
{
  "part": "praise",
  "good_moves": [
    {"move_number": 33, "move": "J5", "kind_label": "capture", "explanation": "<text>", "rating": "<copied>", "rating_source": "<copied>"}
  ]
}
```

### Template D — concepts agent

```
You are writing the "Go concepts & resources" section. Level: <wording>. Band: <band>.
Your only input is <slice_concepts.json>: the chosen moments (move_number, theme,
concept_label when known), student_profile, and game_info.board_size.

Do, in this order, one entry per chosen moment's concept:
1. Look up the concept row in references/level-ladder.md table 1: take the Go Magic resource
   (URL only from the verified library in references/game-arc-commentary.md) and the Sensei's
   Library page name; fetch the Sensei's page to confirm the URL.
2. Write term, japanese (kanji + romaji from the table in references/go-teaching-concepts.md;
   leave japanese as "" if the table has no entry), description (2-3 sentences, register
   rules for the band), resources: Go Magic first, Sensei's second, at most 3.
3. anecdote: copy one item from the curated list in references/go-teaching-concepts.md
   ("Go anecdotes and proverbs") that fits the concept, or omit the key.

Forbidden: anecdotes, quotations, attributions or romanisations not in the curated list;
resource URLs you did not fetch this session (except verified Go Magic library URLs);
more than 3 resources; referring to specific moves of the game.

Return exactly:
{
  "part": "concepts",
  "concepts_learned": [
    {"term": "<English>", "japanese": "<kanji (romaji)>", "description": "<text>",
     "resources": [{"title": "<title>", "url": "<url>"}], "anecdote": "<copied text>"}
  ]
}
```

---

## Stage 2 — sequential, orchestrator only

1. **Merge.**
   ```
   python scripts/merge_lesson_parts.py parsed.json part_overview.json part_lesson_19.json part_lesson_29.json part_praise.json part_concepts.json -o lesson.json
   ```
   The merge orders lessons by move number, renumbers `fact_checks[].text_path`, dedupes
   `concepts_learned[].resources`, sets `schema_version: 2` and `grounding_version: 1`, and
   runs the validator. Read its output.
2. **Editorial pass** on lesson.json — edit text only, never numbers or moves:
   a. the same takeaway must not appear in overall_feedback, a phase narrative and a lesson;
      keep it in the lesson and cut the others to a pointer ("see Lesson 2");
   b. every narrated sequence shows a colour before every coordinate; if a line has bare
      coordinates, rewrite it from the `*_with_colours` list in that moment's slice;
   c. every Japanese term is glossed inline at its first use in each section;
   d. the level sentence appears once, in overall_feedback, with "plays like";
   e. point names: check each "N-M point" against the coordinate (count from the corner).
3. **Validate.**
   ```
   python scripts/validate_lesson.py parsed.json lesson.json
   ```
   Fix every PROBLEM line. If a problem belongs to one part, re-dispatch **only that agent**
   with its original prompt plus the verbatim validator lines and its previous part; merge
   again. Never patch a puzzle or a fact by hand to silence the validator.
4. **Generate.**
   ```
   python scripts/generate_lesson.py parsed.json lesson.json lesson.html
   ```
   Open the HTML; check the praise cards, the level line, one narrated sequence per lesson and
   each puzzle's wrong-move playback. Deliver the single HTML file.

---

## Single-agent fallback

When no sub-agents are available, one agent runs the same parts in this order, using the same
slices and templates as its own checklists:

1. Stage 0 steps 1–7.
2. Template C (praise) — first, so no later prose invents praise.
3. Template A for each moment, one at a time, each written to its own part file.
4. Template B (overview) — after the lessons, so it can point to them instead of repeating them.
5. Template D (concepts).
6. Stage 2 steps 1–4.
