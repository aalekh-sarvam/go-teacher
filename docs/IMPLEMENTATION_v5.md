# Go Teacher v5 — implementation and validation

## Delivered behavior

The regular game/deep analysis still supplies the main review. An additional teaching stage
uses the same engine process for up to four featured teaching moments. Each receives a pass
comparison, candidate ownership plans, a three-root variation tree with two replies per root
and a further evaluated continuation, initiative estimates, and a restricted local reading
when an eligible low-liberty group exists. Human-model availability and profile selection
control the human examples: one most-likely opponent reply plus played/better 24-ply rollouts
at the student and stronger target profiles, each with an unrestricted strong-engine endpoint.

Search defaults: 300 visits per probe; full legal human-policy sampling at one visit per ply;
24-ply cap or two consecutive passes. No default wall-time cutoff silently removes later
features. Profiles come from the selection, then a recognized SGF rank. The target defaults to
two ranks stronger. Unknown opponent rank is explicitly labeled as the student's rank assumption.
No profile selection means human rollouts may be unavailable; it does not invent a rank.

Difficulty is derived for each student position. Up to four additional endgame positions get
pass-based candidate comparisons. Missed opportunities include giving back an opponent's gift
and candidate-plan group-ownership differences. Per-phase likelihood compares a finite set of
human profiles on up to 16 sampled student moves per phase, requiring at least eight samples
for a reported closest profile. This is descriptive similarity, not a calibrated rank estimate.

The pipeline records query count, timings, cache hits and unavailable reasons in the JSON.
Progress identifies the current teaching search and game move; hypothetical positions never
overwrite the main live board. Queries have cancellation and timeout cleanup. Existing batch
queue, resume, heatmaps, JSON cleanup and detailed per-move reports remain available.

## Report and teaching skill

The default Markdown is format 5: a short reading guide and shortlist, then one self-contained
structured evidence block. Heatmap arrays and per-ply sampler telemetry remain in the full
JSON. The original detailed format-4 report is written as `*.detailed.md` and is downloadable
from the app even after JSON cleanup. The in-app report viewer collapses the machine block.

The teaching skill accepts formats 2–5. Format 5 uses lesson schema 2: the model writes prose,
selects move references and researches new quizzes; the generator joins scores, stones and
sequences directly. An optional brief reading view reduces the model's initial reading load.
The report still contains the complete moves and setup needed for an offline lesson.

Lesson controls are six panels plus one page-wide Engine / Your level / Target level selector.
A panel immediately displays a short numbered continuation; the shared stepper and range
control explore the rest. Alternatives have a branch selector, and the human played panel also
exposes the most likely opponent reply with its distinct engine continuation label. The game
arc, story, praise, principles, external resources and practice boards remain.

The skill directs unrestricted external research for non-obvious quiz variations. Game-board
reuse, including symmetry/colour swaps, is rejected. Correct lines and wrong-move refutations
are replayed with capture, pass, suicide and ko checks. Numeric puzzle losses require their own
source. Legality validation does not claim to prove a puzzle's best move.

## Validation performed

- 12 Rust tests passed: SGF branch selection, setup, coordinates, both student colours, full
  fake-engine evidence pipeline, White-handicap human rollouts, pass-value sign identity,
  deterministic legal-policy sampling, capture/ko and timeout/cancel recovery on the same engine.
- 11 skill regression tests passed: old/new report parsing, malformed blocks, exact evidence
  references, forbidden numerical transcription, setup/White/pass, capture/ko, puzzle source and
  solution requirements, and safe script embedding.
- Skill frontmatter validation passed.
- Live Metal smoke test passed using `/opt/homebrew/bin/katago`, the user's existing analysis
  configuration, transformer `b11c768h12nbt3tflrs-fson-silu`, and installed human model. A small
  six-move game exercised 96 additional queries with 20-ply human examples at smoke-test visits;
  every returned sequence replayed legally. Total including model startup: about 110 seconds.
  This validates integration, not the tactical accuracy of the production 300-visit probes.
- Browser checks confirmed engine/student/target switching, immediate numbered sequences,
  branch choices and shared playback. The preview is labeled as synthetic test data.
- Compact-report examples: about 15 KB for the six-move live smoke game and 27 KB for the
  20-move synthetic regression game. New evidence can make a short game's report larger than
  its old detailed report; the improvement is richer selected evidence without per-move bulk,
  not a guarantee that every report shrinks.

## Preserved limits

No second evaluation network, style vector, opponent-adjusted severity, cross-game pattern
mining or thread tuning was added. Configured engine/model paths and thread settings are unchanged.

Restricted reading does not prove life/death, ko or seki. Initiative and ownership are heuristics.
Sampled future differences are not causal move values. One-game policy similarity is not a rank.
The lesson renderer currently supports square boards and rejects rectangular reports explicitly.
Engine and skill regression tests do not validate the quality of a future model-authored lesson;
the model must still research and verify the new quizzes it creates.

## Packaged artifacts

`dist/GoTeacher-0.1.0.dmg` was rebuilt and its checksum verified. The app passes strict code-signature verification (ad hoc, not notarized). AppIcon.icns is included for both the app and SGF document type; SGF registration uses a specific imported file type rather than generic public.data. The signed executable’s code and constant sections match the release build. `skill_indus/go-game-teacher-v5.zip` contains the current validated skill and regression fixtures.
