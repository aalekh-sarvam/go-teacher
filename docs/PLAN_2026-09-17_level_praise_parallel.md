# Plan: level-tailored lessons, praise, parallel skill workflow, and the "no obvious move" bug

Written 2026-09-17 after reading the current code (commits through `eb54585`), the skill folder
and its zips, and the evaluation set in `evals/20260917_evals/` (9x9, Human vs KaTrain 20k bot,
66 moves, B+74.5; the report, detailed report, JSON, lesson HTML and `progress.json`).
The coverage handoff document was deliberately not used as input.

Two lists follow: **A**, the plan for the four observations; **B**, independent suggestions.

---

## Ground truth established during the review

- **Basis.** The skill to improve is `evals/20260917_evals/go-game-teacher.zip` (the one that produced
  the eval lesson); it was unpacked over `skill_indus/go-game-teacher/` as the working copy. Earlier
  skill folders and zips are not used.
- **Praise never fires in this game and cannot.** `teaching::praise()` requires: student's move,
  KataGo rank #1, Black winrate before the move within 5–95%, the second candidate having ≥5 visits,
  and a ≥2-point gap to that second candidate; at most 3 results, decided on whichever pass last
  analysed the position (praise positions are not deepened). In the eval game the winrate left the
  5–95% band at move 12 and never returned. Move 33 (Black J5, captured 6 stones, KataGo #1, second
  candidate 62.9 points worse with 1 visit) and move 31 (H4, #1, 45.6 points better than the next
  candidate, 1 visit) both fail on the window and on the visit rule. The rule "second candidate must
  have ≥5 visits" rejects exactly the positions where one move is overwhelmingly best, because
  KataGo puts 99% of its visits on it.
- **Captures are invisible.** `status_changes` records `captured` only for groups the engine still
  rated alive. The student's captures of 6, 5 and 8 stones (moves 33, 55, 59) appear nowhere; the
  "Stone captured?" column says "no" on every row and the lesson then wrote "nothing else changed
  hands" in the endgame.
- **The level label is not measured.** "Your level · 20k" comes from the upload default profile,
  which happened to equal the *opponent's* SGF rank. A `rank_fit` computation exists (human-policy
  log-likelihood over ≤16 sampled moves per phase, eight profiles at 1 visit) but is emitted only
  as JSON with a "not a rank" caveat and is used by nothing.
- **The lesson hallucinated where the report was silent**: invented praise ("B3 at move 49 was praised
  by the engine"), colour swaps when narrating engine lines (D7, D3, D2, C8, D8 attributed to White),
  "2-1 point" for a 2-3 point, and a history that disagrees with the report because `progress.json`
  counts the same game four times (`file_stem` is empty in every entry).
- `difficulty` labels 1000-visit positions with 98% of visits on one move as "insufficient search";
  the label conflates "one clear move" with "under-searched".

---

# A. Plan for the four observations

Each item says whether it is app-side (Rust, report) or skill-side, and what is verified how.

## A1. Tailor lessons to the player's capacity

### A1.1 Judge the playing rank cheaply (app)

Reuse the human-style network, which we already run, as the estimator. Cost is one network
evaluation per position per profile, no search.

1. **Profile ladder.** Query all student positions (not 16 per phase) with a ladder of eleven
   profiles: `rank_20k, 15k, 12k, 10k, 8k, 6k, 5k, 3k, 1k, 1d, 3d`. One query per profile with
   `analyzeTurns` = every student turn, `maxVisits 1`. On a 66-move game that is 11 queries × 33
   positions at ~0.1 s = about 35 s; on a 250-move game about 2.5 min. Cap at 60 positions per
   game by stride sampling if the game is longer, keeping cost under a minute.
2. **Estimate.** For each profile, mean log-likelihood of the student's actual moves. Convert to a
   likelihood-weighted rank on a numeric scale (20k = −20 … 1k = −1, 1d = 1 … 3d = 3). Report the
   weighted mean and the 68% band from the likelihood spread, rounded to whole stones, per phase
   and overall. Present it as "plays like a 12k (range 15k–10k) in this game"; never as a rating.
3. **Priors and smoothing.** Use the student's SGF rank (`BR`/`WR` for the student's colour) as a
   prior when present. Maintain a **working rank** in `progress.json`: exponentially weighted mean
   over the last five games' estimates. The working rank, not the single game, drives the lesson.
4. **Drive the profiles from it.** The student human profile defaults to the working rank
   (rounded to the nearest available profile), the target to two or three stones stronger, the
   opponent to its SGF rank when present. The upload dropdown becomes an override.
5. **Report.** New `## Student profile` section in the main report and in the evidence JSON:
   per-phase and overall estimate with band, sample count, the upload/opponent profiles actually
   used, and the working rank trend from history. Keep the caveat sentence ("similarity to human
   play at these ranks in this game, not a rating").

Verification: fake-engine test asserts the section exists and the estimate is monotone with the
canned policy; live check on the eval game and on two earlier games to see the estimate is stable
within two stones.

### A1.2 Select lessons and puzzles by rank and arc (skill, with report support)

1. **Concept ladder** (new reference `references/level-ladder.md`): map each theme to concepts with
   a rank band where they are learnable, e.g. 20k–15k: atari, escape, ladder/net, connecting, two
   eyes, counting liberties; 14k–9k: cutting/connecting shapes, capturing races, eye shape (straight
   three, bent four), simple invasions, sente/gote; 8k–3k: direction of play, thickness, aji,
   endgame counting, ko basics. Each concept lists the Go Magic course/lesson (from the verified
   library), the Sensei's page, and the tsumego collection tier that fits.
2. **Selection rule.** From the teaching candidates, choose the 2–3 moments whose concept is at or
   one band above the working rank and whose phase the arc marks as the weakest; break ties by point
   loss in the undecided part of the game. A candidate two bands above the rank becomes a one-line
   "later" note rather than a lesson.
3. **Puzzle difficulty.** Puzzles come from the collection tier matching the band (see A1.3), with
   exactly one tier-up puzzle when the game shows the concept was already handled well once.
4. **Prose register.** Below 10k: define every Japanese term inline the first time, no engine jargon
   ("ownership plan", "gote-like"), and every sequence written with colours ("B D7, W E3").

### A1.3 External puzzle sources and parsing (research + skill)

Run a short web research task (part of the plan, not of the lesson) to confirm licences and access
for these candidates, then wire the ones that pass:

| Source | Why | Format | Parser needed |
|---|---|---|---|
| Tasuki's tsumego collections (Cho Chikun elementary/intermediate/advanced, Gokyo Shumyo) | rank-tiered, public, used already in the eval | LaTeX `.tex` | exists in `current.zip` (`parse_tasuki_tex.py`) |
| OGS puzzle API (`/api/v1/puzzles`, collections with `puzzle_rank`) | rank-tagged, JSON, CC-licensed collections | JSON with SGF-like move trees | **new**: `parse_ogs_puzzle.py` → puzzle dict |
| Sensei's Library beginner exercises and shape pages | named classics for transformation | HTML diagrams / SGF links | existing web reads; add `parse_sgf_problem.py` for SGF attachments |
| Go Magic free lessons/courses | beginner video framing, already the first link | pages, not positions | none (resources only) |
| GoProblems.com | rated problems | JavaScript-rendered | skip unless an export exists |

Decisions: add a tiny `parse_sgf_problem.py` (SGF root setup `AB`/`AW`, `PL`, first correct line) since
most collections ship SGF; add `parse_ogs_puzzle.py` after confirming the API terms; keep the Tasuki
parser. All parsers emit the same puzzle dict that `check_puzzle` already validates, so the
grounding chain is unchanged.

### A1.4 Escalate sources as performance improves

The working rank moves the band; the band moves the collection tier automatically (A1.2, A1.3).
Add two rules: when the same theme has been taught in two of the last three games and the student
now loses fewer points on it, move that concept's puzzles up one tier; when a recurring theme keeps
losing points, hold the tier and switch to a different collection for variety. Record the tier used
per game in `progress.json` so the skill can see it.

## A2. Parallel sub-agent workflow inside the skill

The dependency graph of the current workflow (from the code, not the prose):

- **Sequential, must come first**: parse (`parse_review.py` → parsed/brief/facts), identify student,
  choose the moments (cross-lesson diversity is a global decision), decide the research plan.
- **Independent once the moments are fixed**: one agent per moment doing research + lesson prose
  + fact checks + its puzzle (research seeds both, so they belong together); the overview/arc/
  progress agent (phase searches); the praise agent; the concepts/resources agent (needs only the
  themes and the static Go Magic library, plus links returned by lesson agents).
- **Sequential at the end**: merge (renumber `fact_checks.text_path` array indices), the editorial
  coherence pass (no repeated takeaway across overview, arc and lessons; distinct themes), run
  `validate_lesson.py`, run `generate_lesson.py`; on failure, re-dispatch only the agent that owns
  the failing part with the validator's lines.

Plan:

1. **`references/orchestration.md`**: the stage plan above with one prompt template per agent,
   listing exactly which slices of parsed.json each agent receives (its candidate's `evidence`,
   `move_details[n]`, `facts.lessons[n]`, the arc, the profiles and working rank), and the output
   JSON schema each returns. Fallback: the same stages run sequentially by one agent.
2. **Slicing script** `scripts/slice_for_agent.py parsed.json --moment 19` writes a small JSON for
   one lesson agent (typically 10–20 KB instead of 131 KB brief), and `--role overview|praise|
   concepts` for the others. This is the main speed win and reduces hallucination.
3. **Merge script** `scripts/merge_lesson_parts.py` that assembles agents' outputs into
   lesson.json, renumbers fact-check paths, deduplicates resources, and runs the validator.
4. **SKILL.md** gets a short "Run as a team when the platform allows it" section pointing to the
   orchestration reference; the numbered steps stay for single-agent runs.

Quality guard: the editorial pass and the moment selection are not parallelised; per-lesson agents
never write the overview; every agent's output goes through the same validator.

## A3. Praise good moves, with a grounded rating

### A3.1 App: make praise fire on the moves that deserve it

Rewrite `teaching::praise()`:

1. Drop the 5–95% winrate window. Replace it with a *point* view: the move counts if it was the
   student's, not a pass, and either (a) KataGo rank #1 with the best *other* searched move at
   least 2 points worse (compare scores across all other candidates regardless of visits; a
   1-visit alternative that KataGo abandoned is evidence it was worse, not missing evidence), or
   (b) it captured ≥3 stones or turned an opponent group from alive/unsettled to dead, or saved
   the student's own group from dead to alive, with point loss ≤ 1.0.
2. Add a **non-obvious** flag: the played move is best (loss ≤ 0.5) and the human policy at the
   student's rank gives it under 25%. That is the user's "not obvious for a beginner" case and the
   first thing to praise.
3. Decide praise on deep data: add praise positions (before and after) to `deep_turns`.
4. Return up to 5, guarantee at least 3 when any qualify by relaxing thresholds in order
   (gap ≥1.5, then top-1 in a "several choices" position), and tag each with its kind: only move,
   capture/kill, save, non-obvious, steady.
5. **Rating**: from the profile ladder in A1.1, record for each praised move the weakest rank at
   which the played move is the most common human choice (≥30% policy). Report "a move most 8k
   players find" as the rating with `rating_source: human-policy ladder`, which satisfies the skill's
   rule that ratings need a source. This replaces invented kyu badges.
6. Fix the capture ledger: count stones removed by every move from the board replay and record
   captures of already-dead groups as `captured (already dead)`; store `stones_captured` on each
   review row and in the compact move list.
7. Main report: add a visible `### Good moves` block (currently praise reaches the agent only as a
   JSON array). Remove the sentence "No move stood out as the only good move" whenever any capture
   or gain exists; print the kinds instead.

Verification: fake-engine test with a canned 6-stone capture at 99% winrate must produce a praise
entry of kind "capture"; live run on the eval game must praise moves 33, 31 and 59 with ratings.

### A3.2 Skill: a praise sub-agent and 3–4 examples per lesson

1. Praise agent input: `teaching.praise` (now populated) plus the moves timeline with
   `stones_captured` and gains. Output: 3–4 `good_moves`, each with move number, what it achieved
   (stones captured, group saved, gap to the alternatives), why a beginner would not see it (human
   policy figure), the rating and its source, and a one-line takeaway. The generator already renders
   these with the board; extend it to show the rating source under the badge.
2. The grounding checker already rejects a `good_moves` entry whose move is not the student's;
   add checks that the explanation's numbers (stones, gap, policy) match the evidence, using the
   same `{{fact:}}` mechanism as lessons.

## A4. Why the report said no move was obviously best, and the fix

Root cause is the praise filter in A3.1 (winrate window plus the ≥5-visit rule), compounded by
praise being judged on the shallow first pass and the `difficulty` labeller. Concretely for the
eval game: move 31 H4 had 997 of 1003 visits on the played move and the next candidate at 1 visit;
move 33 J5 had 144 of 153 visits with the next candidate at 1 visit and 62.9 points worse; both
positions were at 99.97% Black winrate. The filter therefore reported "no move stood out".

Fixes beyond A3.1:

1. `difficulty` labels: "one clear move" when the top candidate holds ≥90% of visits and the gap
   is ≥2 points; "under-searched" only when total root visits are below 100; "several choices"
   otherwise. Print the label next to each teaching candidate and praised move.
2. Turning points: only report a swing when both endpoints were deep-analysed, or the loss is
   ≥3 points, so 150-visit noise on an empty board ("Move 2 changed the lead") disappears.
3. Theme rules: a move that puts an opponent group in atari or captures is "attack/reading", not
   "over-defending"; a move that reduces eye space or is a killing placement is "life and death".
   Use the new capture ledger and the atari list in the rule order.

## Order of work

1. Working skill = the eval zip, unpacked (done).
2. App: capture ledger, praise rewrite, deep-turn inclusion, difficulty labels, theme fixes,
   Student profile section and rank ladder (A3.1, A4, A1.1). Bump the report format; parser accepts it.
3. Skill: level ladder reference, praise agent, orchestration reference, slicing and merge
   scripts, SGF/OGS puzzle parsers, prose register rules; regression tests on the eval report.
4. Re-run the eval game through the app and the skill; check the acceptance list: moves 31/33/59
   praised with ratings, "12k-ish" style estimate present, no invented praise, colours correct in
   every narrated sequence, puzzles from the matching tier.
5. Repack the skill zip.

---

# B. Independent suggestions

1. **Write every sequence with colours in the report** ("B D7 W E3 B D3 …") and give the generator a
   single `render_sequence()` that prints the same tokens. The colour swaps in the eval lesson came
   from bare coordinate lists; the model cannot alternate colours reliably.
2. **Use the SGF undo branches as evidence.** The eval SGF shows the student tried D3 before F1 and
   D1/H1 before H4. Record the discarded tries per move ("also considered: D3") in the JSON and let
   the lesson say "you considered D3 first". It is the only direct signal of what the student was
   thinking.
3. **Decided-game framing.** When the winrate has been outside 5–95% for 20+ moves, print losses
   relative to the margin and let the report say plainly that the game was won by move 12 and the
   later "blunders" were about how much, not whether. It changes lesson selection toward the moves
   that decided the game.
4. **Deduplicate `progress.json`** by game identity (date, players, move count, result) instead of
   analysis timestamp, and fill `file_stem`. The report and the lesson currently disagree on the
   history.
5. **Validator parity with the generator.** The generator raises `KeyError` on several keys the
   validator does not check (puzzle `title`, `hint`, `player_to_move`; `good_moves.explanation`;
   `concepts_learned` fields). Add them to the contract so "0 problems" means the HTML builds.
6. **Fix the stale UI documentation** in `html-build-guide.md` and SKILL.md (they describe the old
   buttons), remove the dead `alt_buttons` code, and stop the skill asking for fields schema 2
   forbids (`alternatives`, `played_quality`, `concept`).
7. **Markdown in panels**: panel prose is inserted as text, so `**bold**` shows literally; route it
   through the same formatter as static prose.
8. **Non-featured candidates** (3 of 7 in the eval) carry `unavailable.deeper_search` yet still have
   refutations and better lines; tell the skill they are valid lesson targets with reduced panels,
   or feature all final candidates.
9. **Puzzle/lesson theme consistency check**: the eval taught an outside hane at the 2-3 point as a
   "nakade" and practised straight-three. Add a validator warning when a puzzle's `evidence_focus`
   or concept label does not match the lesson candidate's theme.
10. **Facts output per moment** (`--moves`) from `parse_review.py`, so facts.json is not 113 KB.
11. **Reading time for a 20k**: the lesson used semeai, nakade, kyūsho, "restricted search prediction
    −0.998". Add a jargon list the generator can auto-gloss on first use.
12. **Progress card dedupe and the "Your progress" numbers** should come from the same source as the
    report's history section once B4 is done; until then the skill should quote the report only.
13. **Rank-fit endgame oddity**: with too few moves the best-fitting profile was 3d; suppress phase
    estimates below the sample threshold entirely instead of printing them with a flag.
14. **Anecdotes and proverbs** in the resources section should be limited to the curated list in the
    concepts reference; the eval produced a garbled romanisation and unsupported attributions.
