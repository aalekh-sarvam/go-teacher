# Game Arc Commentary

How to read a single game as a story rather than a list of disconnected mistakes, and how to enrich that story with sequential web research. The tactical lessons (in `go-teaching-concepts.md` and the SKILL.md workflow) stay exactly as they are; this reference adds a *holistic* layer on top of them — the arc of the game.

## Table of contents

1. [What "arc" commentary is](#what-arc-commentary-is)
2. [Segment the game into phases](#segment-phases)
3. [Trace cause→effect chains around each shortlisted move](#trace-chains)
4. [Sequential web searches](#sequential-searches)
5. [Writing the arc narrative](#writing-arc)
6. [Board-fact discipline for arc claims](#arc-board-facts)
7. [Verified Go Magic tutorial library](#gomagic-library)

---

## What arc commentary is

The teaching candidates are isolated snapshots: "at move 23 you should have played E7." A beginner often can't place that snapshot in context. Arc commentary tells them how they *got* to move 23 and what happened *because* of it — the cause-and-effect thread that runs through the whole game.

Think of it as three layers, each feeding the next:

1. **The phase layer** — early / middle / late game: what each side was trying to do in that stretch, and how the strategic balance shifted.
2. **The chain layer** — local cause→effect sequences that span several moves: the atari at move 19 that forced a connection at 20, which left the right-side group thin, which is *why* the invasion at 31 was a mistake. Shortlisted teaching moves are usually the visible tip of a chain; trace the chain both backward (how the position arose) and forward (what the mistake led to).
3. **The pattern layer** — each phase and chain maps onto well-known Go patterns and proverbs (playing under strong stones, premature invasion, urgent-before-big, ladder direction). Research these online to give memorable names and remedies.

Arc commentary supports the tactical lessons. Keep the existing lesson, praise and practice sections; choose only the broad observations that help the student improve. For format 5, read the optional compact-context contract in `evidence-format5.md`: it explains the scored timeline, statistics, missing data and how to avoid repeating the overview in every phase. There is no need to narrate every move or force a named pattern onto every phase.

---

## Segment the game into phases

Use `arc.phases` from the parsed review: it already gives each phase's move range, the winrate and score at both ends, mean loss per side, the worst move per side, the student's top-1 rate and the hot regions where the board changed most. Add `arc.status_changes` for detected ownership-prediction changes and recorded capture events in that stretch. Ownership labels do not prove life or death; this is not a complete group-status or capture ledger. These numbers, plus the compact move list, are the phase story's facts. On a 9×9 the phases are short; on 19×19 they are longer. The program uses move-count heuristics, not board-based phase recognition. The following are conceptual descriptions for the teacher, not the implemented cut-offs:

- **Opening (fuseki)** — from the first move until corners and sides are roughly claimed and the first serious contact occurs. On 9×9 this may be only the first 6–10 moves. Focus: direction of play, big points, who took which framework, any immediate tactical blunders.
- **Middle game (chūban)** — from first contact / fighting until territory borders harden. This is where most teaching candidates live. Focus: local fights, invasions, shape battles, atari and capture chains, groups that got heavy or cut.
- **Endgame (yose)** — borders set, moves are about counting and order. Focus: sente endgame, point values, whether the student played small points first.

For each phase, briefly note how the strategic balance shifted and which decisions matter to the lesson. Support claims about plans with the actual teaching evidence; do not infer intentions from the score trajectory alone. If a phase was uneventful, say so in one line — don't pad.

The free Go Magic lesson "Three Stages of the Game" (see the library below) is a good model for how teachers frame these transitions.

---

## Trace cause→effect chains around each shortlisted move

For every teaching candidate, build a short chain narrative. The parsed review gives you the chain directly: `chain_before` and `chain_after` (moves in the same area, each with its point loss and notes such as `captured at move N` or a status change), `status_changes` on the candidate, and `refutation` (what the mistake allows). Use them as follows:

- **Backward** — what created the position. Read `chain_before`: which earlier move in the same area first lost points? Was a group already unsettled (status changes in the chain)? Read the compact move list and the per-move scalar facts for the 4–6 moves *before* the candidate. Which group was threatened? Which stones were already heavy or cut? Was there a tenuki that left something hanging? Use the `loss_region` and the atari/tenuki hints on earlier moves. The mistake at the candidate move is rarely the first error in the chain; often the real decision was two or three moves earlier.
- **Forward** — what the mistake *caused*. Use `refutation` for what the move allowed in principle, and `chain_after`, `captured_at` and the status changes for what actually happened. "The atari at 23 was answered too late; by 27 the group had to sacrifice two stones, and that sacrifice is what made White's centre thick — which is why your 31 invasion failed."

Aim for one concrete chain per lesson, 2–4 sentences, grounded in move numbers the student can follow. Name the *relationship* between moves ("this is the price of", "this is the follow-up to", "this is why"), not just the moves in isolation.

Chains connect the tactical lesson to the game's flow and are the single most valuable thing arc commentary adds.

---

## Sequential web searches

Research is not one search per mistake — it is a short *sequence* of searches that follows the game's development. Do this in addition to the per-mistake pattern searches already in the workflow:

1. **Phase searches.** One search per phase the student struggled in, phrased around what *actually happened*: e.g. "9x9 Go opening playing too close to opponent strength beginner", "Go middle game invading while group is heavy", "Go endgame sente moves first". Read the two best results from each and distil 2–3 bullets of teacher phrasing.
2. **Chain searches.** Search the actual tactical chain you traced: "Go ignoring atari then group captured sequence", "Go ladder breaker then ladder fails", "Go cutting stone no support sacrifice". You are looking for a named pattern and the standard remedy — confirm the pattern name and capture the corrective advice.
3. **Pattern name searches.** Once a chain maps to a shape or proverb (empty triangle, bamboo joint, playing under strong stones, premature invasion, urgent before big), search that name + "Go" + "beginner" to find the clearest explanation and a classic example position (these seed the puzzles).
4. **Pro / classical example searches.** For the strongest teaching moments, look for a well-known game or tsumego that illustrates the same chain. Sensei's Library, Go Magic, and Go books are the best sources. Note the shape, not the coordinates.
5. **Go Magic, specifically.** Before defaulting to a generic source, check the [verified library](#gomagic-library) below for a course or free lesson that covers the exact concept, and cite it. Go Magic's video lessons are unusually beginner-friendly and pair well with this skill's tone. When a course fits, prefer it as the first resource link.

Distil 2–4 bullets of researched phrasing per chain, plus the matched Go Magic link. Research is enrichment, but do it thoroughly — the names, proverbs, and examples are what make the commentary better than the raw numbers.

---

## Writing the arc narrative

The output gains two new slots (see `html-build-guide.md`):

- **`game_arc`** — a top-level object with a short `intro` and a `phases` array (one entry each for Opening / Middle game / Endgame that the game actually had). Each phase: `phase`, `move_range`, a brief `narrative`, and an optional `turning_points` list of short move-tagged notes. This is the holistic commentary that sits above the individual lessons.
- **`story`** — a per-lesson field: 2–4 sentences tracing the cause→effect chain around that move (how the position arose, what the mistake led to). This sits with each tactical lesson so the snapshot has context.

Voice and style for both:

- Write as a narrator watching the game unfold, then dropping in to teach. Past tense for what happened ("White built thickness on the right"), present tense for the principle ("When your group is heavy, don't invade").
- Always anchor in move numbers the student can find in the compact move list.
- Every narrated move carries its colour, copied from the record: actual moves as `moves[].color` + `move` ("B G4 at 19, W F3 at 22"), engine lines as `*_with_colours` tokens. Never write a bare coordinate list and never alternate colours yourself.
- The arc never praises a move: praise lives in `good_moves` and comes only from `teaching.praise` (SKILL.md step 8). The arc may say a move "settled the corner" only when a status or capture fact supports it.
- Connect, don't list. Prefer "because", "so", "which is why", "the price of" over a bare sequence of move references.
- Keep it honest about uncertainty: a saturated winrate does not mean the human game was decided. Check point losses and later score changes; do not dismiss a reversal as noise merely because the earlier winrate was near 0% or 100%.
- Never invent captures, ladders, atari, or ko the report doesn't support (see below).

---

## Arc board-fact discipline

Everything in `references/go-teaching-concepts.md` and SKILL.md's "Board-fact discipline" applies to arc claims too — and arc claims are the easiest place to fabricate, because you're narrating across many moves. In particular:

- Only claim a capture from an explicit capture fact: `moves[].stones_captured > 0` (evidence version 2, the complete ledger — quote the count), a `captured_at` note, or an `arc.status_changes` row. Coordinates or a score change alone do not establish a capture, and a capture alone does not establish that a different capture was missed. The eval lesson wrote "nothing else changed hands" in an endgame with 5- and 8-stone captures because it read only the status table; check the ledger.
- Only claim atari or a liberty count from the hints or a diagram you can see — don't infer "this group was in atari" by imagination.
- Only claim a tenuki if the move list shows the player played elsewhere while a threat was on the board.
- `loss_region` tells you *where* points changed; use it to say where, not to invent a tactic.
- If you want to illustrate a chain tactic that the PV doesn't show, say explicitly that it is your own illustration, not the engine's line.

---

## Verified Go Magic tutorial library

Go Magic (gomagic.org) is an interactive Go learning platform with free and paid video courses. These URLs were verified at the time this reference was written and are the **only Go Magic URLs a lesson may cite without fetching**; `level-ladder.md` table 1 names its Go Magic resources from this library. **Re-verify a slug before citing if you haven't fetched it this session** — courses are occasionally re-slugged. The fastest check is a web search restricted to `gomagic.org`; the courses index is https://gomagic.org/courses/ . Prefer free courses and free lessons for resource links; note paid courses honestly ("paid course, free trial lesson").

Map each mistake theme / arc chain to the best Go Magic resource:

### Fundamentals & overall game (use for opening and as a general resource)

| Topic | Resource | URL |
|---|---|---|
| Rules + liberties, atari, eyes, scoring (free course) | The Rules of Go | https://gomagic.org/courses/go-rules/ |
| Core strategy, snapback, net, cutting/connecting, good/bad shape, ko, life & death (paid, free trial) | The Fundamentals of Go on 13×13 — Remake | https://gomagic.org/courses/the-fundamentals-of-go-on-13x13/ |
| First steps on 19×19 (paid) | Deeper into the Game of Go on 19×19 | https://gomagic.org/courses/deeper-into-the-game-of-go-on-19x19/ |
| Beginner game reviews on 9×9 & 13×13 (paid, free trial lessons) | Game Reviews for Beginners | https://gomagic.org/courses/game-reviews/ |
| Beginner's guide + study plan (free article) | Go for Beginners: Online Guide | https://gomagic.org/beginners-guide/ |
| Strategy & tactics overview, ladders, capturing races (free article) | Go Strategy & Tactics | https://gomagic.org/go-strategy-and-tactics/ |

### Free single lessons (great for the phase layer — direct links, no paywall)

| Topic | Lesson | URL |
|---|---|---|
| Opening / middle / endgame transitions — **the model for phase segmentation** | Three Stages of the Game | https://gomagic.org/lessons/three-stages-of-the-game/ |
| Where to play first, direction & priorities | Where Do I Begin? | https://gomagic.org/lessons/where-do-i-begin/ |
| When to attack vs defend, sente/gote feel | To Attack or to Defend? | https://gomagic.org/lessons/to-attack-or-to-defend/ |
| Why invade vs reduce — frames invasion-timing lessons | Why Do We Need to Invade? | https://gomagic.org/lessons/why-do-we-need-to-invade/ |

### Theme-specific courses (map to mistake themes & chains)

| Mistake theme / chain | Go Magic course | URL |
|---|---|---|
| Reading & tactics: ladders, ladder breakers, ladder direction | Ladders: When and How | https://gomagic.org/courses/ladders/ |
| Capturing-race basics, atari technique drills (free mini-game) | Atari Go (Capture Go) | https://gomagic.org/atari-go/ |
| Common beginner mistakes: automatic atari, bad tesuji, edge descent, premature endgame, moyo defence (**free**, pairs directly with this skill's lessons) | Typical Mistakes That Wouldn't Let Us Get Better | https://gomagic.org/courses/typical-mistakes/ |
| Higher-kyu mistakes: atari handling, tenuki when threatened, bad shape, aji, heavy groups, pointless moves (paid, free trial) | Typical Mistakes 2.0 | https://gomagic.org/courses/typical-mistakes-2/ |
| Direction of play, big vs urgent moves, tenuki, move efficiency, AI vs classic opening (paid, free trial) | The Main Principles of a Glorious Opening | https://gomagic.org/courses/the-main-principles-of-a-glorious-opening/ |
| Invasion timing: invade vs reduce, side & corner invasions, sabaki, handling unreasonable invasions (paid, free trial) | Fearless Invasions | https://gomagic.org/courses/basic-invasions/ |
| 3-3 invasions of star-point corners (paid, free trial) | Common 3-3 Invasions in Actual Games | https://gomagic.org/courses/common-3-3-invasions-in-actual-games/ |
| Endgame counting, sente values, move order, follow-up value (paid, free trial lessons) | How NOT to Lose 30 Points in the Endgame | https://gomagic.org/courses/how-not-to-lose-30-points-in-the-endgame/ |

### Other catalogue courses (verify the slug before citing)

These appear on the Go Magic courses index but their slugs were not individually fetched here — search gomagic.org for the exact title before linking: Go Shapes (Level 1 / Level 2), Moving to the Middle Game, Attack and Defense in the Middle Game, Endgame for Nerds, Ko — Take Control Over the Chaos, Forcing Moves: the Good, the Bad and the Ugly, The Art of Attachments, Defensive Tesujis In a Nutshell, Life and Death vol.1 / vol.2, Aji — The Phantom Potential, Sabaki: The Elegant Dance of Survival, Score Estimation (free), Probing Moves, Joseki ABCs, Finding the Right Joseki at the Right Time, Basics of the 5-3 Point, Go Forth in Style. Use these for shape, ko, aji, sente/gote, tesuji, and joseki lessons where they fit better than the courses above.

### Pairing with other sources

Go Magic pairs well with Sensei's Library (https://senseis.xmp.net/ ) for pattern names and proverbs, and with the tiered tsumego sources in `level-ladder.md` table 2 for drill positions. For each `concepts_learned` entry, list the single best Go Magic link first, one Sensei's Library link second (fetched this session), and never more than 3 resources. Anecdotes come only from the curated list in `go-teaching-concepts.md`.
