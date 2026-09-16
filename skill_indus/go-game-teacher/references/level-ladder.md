# Level ladder: what to teach at which rank

Use this to turn `student_profile` into three decisions: which moments become lessons, which
puzzle tier to draw from, and how to write. All rank labels are the report's (`20k` … `1k`,
`1d` …). The estimate is a similarity to human play, not a rating; always say "plays like".

## Rank label → band

| Band | Rank labels | Name used below |
|---|---|---|
| 1 | 20k, 19k, 18k, 17k, 16k, 15k | **20k–15k** |
| 2 | 14k, 13k, 12k, 11k, 10k, 9k | **14k–9k** |
| 3 | 8k, 7k, 6k, 5k, 4k, 3k | **8k–3k** |
| 4 | 2k, 1k, 1d and stronger | **2k+** |

Which label to use, in order: `student_profile.working_rank.rank_label` (smoothed over the
last games) → `student_profile.estimate.rank_label` (this game) → `student_profile.profiles_used.student`
(strip `rank_`; say the source from `student_source`) → band 1 if nothing exists. A label
outside the table (weaker than 20k) is band 1.

## Table 1 — concept ladder

`Theme` is the program's label on `teaching.candidates[].theme`. One theme maps to several
concepts; pick the row whose band is at or one above the student's band and whose description
matches the candidate's hints/refutation. Go Magic URLs come only from the verified library in
`game-arc-commentary.md` (section "Verified Go Magic tutorial library"); rows marked *verify*
name a catalogue course whose slug must be searched before linking. Sensei's page = the page
title to search at senseis.xmp.net; fetch it before citing the URL.

| Band | Theme | Concept (concept_label) | Go Magic resource | Sensei's page | Puzzle tier | Objective kind |
|---|---|---|---|---|---|---|
| 20k–15k | reading / tactics | Atari and escape | The Rules of Go (free course); Atari Go (free) | Atari; Escape | T1 | capture_within / avoid_capture_for |
| 20k–15k | reading / tactics | Ladder and net | Ladders: When and How | Ladder; Net | T1 | capture_within |
| 20k–15k | reading / tactics | Counting liberties | The Rules of Go (free course) | Liberty | T1 | capture_within |
| 20k–15k | shape / connection | Connect or cut | The Fundamentals of Go on 13×13 (cutting/connecting) | Connection; Cut | T1 | avoid_capture_for |
| 20k–15k | life and death | Two eyes | The Rules of Go (free course) | Two Eyes; Eye | T1 | avoid_capture_for |
| 20k–15k | tenuki while threatened | Answer the atari / defend before attacking | Typical Mistakes That Wouldn't Let Us Get Better (free) | Tenuki | T1 | avoid_capture_for |
| 20k–15k | priority (played locally, bigger move elsewhere) | Urgent before big (simple) | Where Do I Begin? (free lesson) | Urgent Moves Before Big Moves | T1 or OGS | compare_plans |
| 14k–9k | reading / tactics, life and death | Capturing race (semeai) | Go Strategy & Tactics (free article) | Capturing Race | T1/T2 | capture_within |
| 14k–9k | life and death | Eye shape: straight three, bent four, vital point (nakade) | The Fundamentals of Go on 13×13 (life & death) | Nakade; Straight Three; Bent Four in the Corner | T2 | capture_within / avoid_capture_for |
| 14k–9k | reading / tactics | Snapback and ko basics | The Fundamentals of Go on 13×13 (snapback, ko) | Snapback; Ko | T2 | capture_within |
| 14k–9k | shape / connection | Good and bad shape: empty triangle, bamboo joint, tiger's mouth | The Fundamentals of Go on 13×13 (good/bad shape); Go Shapes Level 1 *verify* | Empty Triangle; Bamboo Joint; Tiger's Mouth | T2 or OGS | compare_plans |
| 14k–9k | priority (played locally, bigger move elsewhere) | Sente and gote; urgent before big | To Attack or to Defend? (free lesson) | Sente; Urgent Moves Before Big Moves | OGS | compare_plans |
| 14k–9k | tenuki while threatened | Weak group first | To Attack or to Defend? (free lesson) | Weak Group | T2 | avoid_capture_for |
| 14k–9k | direction of play | Simple invasion timing; 3-3 invasion | Why Do We Need to Invade? (free lesson) | 3-3 Point Invasion | OGS | compare_plans |
| 8k–3k | direction of play | Direction of play; thickness; big points | The Main Principles of a Glorious Opening | Direction of Play; Thickness | OGS | compare_plans |
| 8k–3k | endgame counting | Endgame values; sente/gote endgame; move order | How NOT to Lose 30 Points in the Endgame | Endgame; Sente Gote Endgame | OGS | compare_plans |
| 8k–3k | direction of play | Invade or reduce; sabaki | Fearless Invasions | Invasion; Reduction; Sabaki | OGS | compare_plans |
| 8k–3k | priority (played locally, bigger move elsewhere), tenuki while threatened | Heavy groups; aji; when to tenuki | Typical Mistakes 2.0 | Heavy; Aji | T2/T3 | compare_plans |
| 8k–3k | life and death | Corner life and death (standard shapes) | Life and Death vol.1 *verify* | Life and Death | T2/T3 | capture_within / avoid_capture_for |
| 8k–3k | shape / connection | Tesuji; attachments | The Art of Attachments *verify*; Defensive Tesujis In a Nutshell *verify* | Tesuji | T3 | capture_within |
| 2k+ | reading / tactics, life and death | Ko fights; complex tsumego | Ko — Take Control Over the Chaos *verify* | Ko Fight | T3 / Gokyo Shumyo | capture_within / compare_plans |
| 2k+ | direction of play, shape / connection | Joseki choice; probes | Finding the Right Joseki at the Right Time *verify*; Probing Moves *verify* | Joseki; Probe | OGS | compare_plans |
| 2k+ | endgame counting | Counting and endgame theory | Endgame for Nerds *verify* | Endgame Counting | OGS | compare_plans |

`unclassified` themes: choose the concept from the candidate's hints (atari → Atari and escape;
capture → Counting liberties; status change → the life-and-death row of the band) and state the
reason in one sentence in the story.

## Table 2 — puzzle tiers

| Tier | Source | Bands | How to get the position |
|---|---|---|---|
| T1 | Tasuki `cho-1` (Cho Chikun, Elementary) | 20k–10k | `scripts/parse_tasuki_tex.py cho-1.tex -o cho-1.json`, then `scripts/solve_tsumego.py` (tsumego-source-formats.md) |
| T2 | Tasuki `cho-2` (Intermediate) | 10k–5k | same, `cho-2.tex` |
| T3 | Tasuki `cho-3` (Advanced); `gokyoshumyo` for 2k+ | above 5k | same, `cho-3.tex` / `gokyoshumyo.tex` |
| OGS | online-go.com puzzle collections filtered by `puzzle_rank` within the band | any | `scripts/parse_ogs_puzzle.py <url or id> -o puzzle.json` |
| SGF | Sensei's Library exercise SGFs; gogameguru weekly problems (easy = bands 1–2, intermediate = 3, hard = 4) | any | `scripts/parse_sgf_problem.py problem.sgf -o puzzle.json` |

Rules:
- A life-and-death or reading lesson takes a tsumego tier (T1–T3) matching the band; a
  direction, priority or endgame lesson takes OGS or SGF with `compare_plans`.
- A band-1 student never gets T2 or above; a band-4 student never gets T1.
- One tier up is allowed for exactly one puzzle when the escalation rule below says so.
- Every puzzle records `source.url`, `source.example_locator` (book + problem number, OGS
  puzzle id, or SGF file + node) and `source.position`, then a transformation that changes the
  reading (grounding-and-practice.md). The tier is a difficulty guide, not a verification.

## Selection rules (Stage 0 of orchestration.md; SKILL.md step 4)

Apply in order to `teaching.candidates`:

1. Map each candidate's `theme` to a concept row (table 1). A candidate whose only fitting rows
   are two or more bands above the student's band is **not** a lesson; keep its move number for
   one "later" sentence in the overview.
2. Prefer candidates whose phase (`arc.phases`, by move range) is the phase with the highest
   student mean loss.
3. Within that, order by `point_loss`; the engine winrate is not a criterion.
4. Never pick two candidates with the same `theme`. If the top two share a theme, take the
   larger loss and continue down the list.
5. Prefer candidates with a non-empty `refutation_with_colours`; a candidate without one can
   still be a lesson if its `better_line_with_colours` exists.
6. Candidates without investigations — `investigation_coverage.status` of `not_selected` /
   `disabled` / `unavailable`, or `evidence.unavailable.teaching_investigations`, or the legacy
   `evidence.unavailable.deeper_search` — are valid lessons with fewer panels (omit `local` and
   `what_if`, write no human reply). These keys say nothing about deep evaluation. When
   `evaluation_coverage.status` is known, prefer `complete` candidates for the primary lessons;
   `incomplete` or `unknown` ones are usable but are never called "verified".
7. Result: 2 lessons, or 3 when a third distinct theme exists at the right band. If fewer
   than 2 qualify, take the largest remaining candidate at any band and say in `level_framing`
   that the idea is ahead of the student's level.

## Prose register per band

Everything is copied colour tokens and glossed terms; the differences are density and vocabulary.

| Band | Register |
|---|---|
| 20k–15k | One idea per sentence. Define every Japanese term inline the first time ("atari — one liberty left"). No engine jargon: never "ownership", "policy", "visits", "restricted search", "rollout", "entropy", signed predictions like "+0.877". Say "KataGo expects" or "the search predicts". Every sequence with colours ("W F3, then B D7"). Name points by counting from the corner ("B3, the 2-3 point"). Numbers: whole points, one per sentence. |
| 14k–9k | Same rules; terms may be reused after the first gloss; two ideas per sentence allowed; probabilities as "about 1 in 20". |
| 8k–3k | Standard Go vocabulary (sente, gote, aji, thickness) with a gloss on first use; engine terms allowed when named plainly ("KataGo's estimate", "a searched line"); decimals allowed. |
| 2k+ | Full vocabulary; engine terms and decimals allowed; still no invented ratings and still colours in every sequence. |

Rules that apply to every band: the level sentence is quoted from `student_profile.estimate.wording`
with "plays like"; every praise rating is copied with its source; nothing about what the student
"saw" or "thought".

## Escalation across games

Use `facts.history.prior_games` and each game's themes/puzzle tiers when they are recorded.

- **Handled well twice**: the same theme appeared in two of the last three games and this game's
  candidates for that theme lose fewer points than before (or the theme is absent) → the one
  puzzle for that concept goes one tier up (T1→T2, T2→T3, OGS `puzzle_rank` one band higher).
- **Recurring**: the same theme is a candidate in three or more of the last games → make it the
  first lesson, hold the tier, and rotate the collection (T1 Tasuki → OGS → SGF) so the student
  does not see the same book twice in a row.
- **No history**: hold the band's tier; no escalation.
- Record in the lesson's puzzle `verification` text which tier and collection were used, so the
  next run can see it.
