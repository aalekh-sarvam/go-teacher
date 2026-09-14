# Go Teacher v5 implementation contract

The saved design/review results are in `v5-reviews/`. This synthesis supersedes their conflicting suggestions, including the suggestion to use actual game positions for puzzles.

## Scope

Keep the existing application, engine configuration, move-by-move review, heatmaps, queue and lesson narrative. Add pass comparisons, endgame values, initiative estimates, ownership plan comparisons, search difficulty, restricted local reading, evaluated variation trees, human-policy replies, 24-ply counterfactuals, missed opportunities and per-phase human-policy likelihood comparisons. Exclude a second evaluation network, style vectors, opponent-adjusted severity, cross-game mining and thread tuning.

## Evidence and limits

All stored scores/winrates/ownership are Black-view. Deltas name their beneficiary and use that player's sign. Pass comparisons are whole-board estimates, not exact local endgame values or mathematical temperature. Never probe a second consecutive pass as urgency. Initiative is an estimate: local punishment is not sente; a local answer alone does not establish a threat. Ownership changes are predictions, not life/death proofs. Restricted searches retain the real history and rules, describe their allowed region and finite depth, and cannot prove ko, seki or global life/death.

Search the selected teaching moments with the existing main engine. Use three root alternatives (including the played move), two evaluated replies and one evaluated continuation per reply. Generate human-policy samples with a recorded deterministic seed, independently for both configured teaching perspectives, evaluating the endpoints with the strong engine. Use the opponent SGF rank when available; otherwise label the assumed opponent profile. Sample full legal policy mass, without a strength-changing probability floor. Stop at two passes or 24 plies; do not stop because an engine winrate has saturated. Rank likelihood is similarity within the tested profiles, never a calibrated rank or credible interval.

Failures/missing models are explicit. Cancellation terminates the query and the job. Probe progress never overwrites actual-game board evaluations with hypothetical positions. Query timeouts clean up registrations. Extra analysis is finite by selected moments and depth, not silently dropped by a default wall-time budget. No research budget is added to the skill.

## Data and UI

`GameAnalysis.probes` stores full computed evidence and arrays in the JSON. A compact report format 5 has one machine-readable evidence block with selected moments and no heatmap arrays; the detailed format-4 report remains available separately. Include setup stones, actual move colours, rule set and sequence start positions. Keep all old report capabilities through the detailed report and JSON.

The parser reads format 2–5, exports full structured data for the generator and an optional brief for the teaching model. Lesson schema 2 uses move/evidence references and prose. The generator hydrates numeric facts and sequences itself. Panels: position, played consequence, better plan, alternatives, local reading, and longer what-if. One perspective control selects engine / your level / target level, and one stepper controls the selected sequence. Missing perspectives say why; never substitute engine moves under a human label.

Puzzles remain researched, non-obvious variations of external examples illustrating the lesson. Never reuse game positions, even by rotation/reflection/colour swap. Require source, transformation, relation to the evidence, and checked solution/refutation lines. Search as broadly as needed. The validator checks references, colours, setup, bounds, captures, suicide, ko and sequence origins before offline HTML generation.

## Validation

Use deterministic fake-engine end-to-end tests for both student colours, setup, pass/cancel/timeout, evidence/report parsing and lesson hydration. Check illegal branches and puzzle refutations with actual captures/ko. Perform a bounded live Metal probe, render the lesson in a browser, and rebuild the app/DMG and skill ZIP. Record material limitations instead of presenting heuristics as established Go facts.
