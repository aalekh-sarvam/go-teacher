//! Compact, self-contained report data. Heatmap arrays stay in the full analysis JSON.
use crate::analysis::GameAnalysis;
use crate::sgf::Color;
use serde_json::{json, Value};
pub fn build(a: &GameAnalysis) -> Value {
    let g = &a.game;
    let student = a.student.unwrap_or(Color::Black);
    let coords =
        |cs: &[crate::sgf::Coord]| cs.iter().map(|c| c.to_gtp(g.size_y)).collect::<Vec<_>>();
    let moves: Vec<_> = a
        .reviews
        .iter()
        .map(|r| json!({"number":r.number,"color":r.color.letter(),"move":r.mv}))
        .collect();
    let candidates: Vec<_> = a
        .teaching
        .iter()
        .map(|t| {
            let mut v = serde_json::to_value(t).unwrap();
            v["move_number"] = json!(t.number);
            v["player"] = json!(t.color.letter());
            v["played_move"] = json!(t.mv);
            v["preferred_move"] = json!(t.best);
            v["theme"] = json!(t.theme.label());
            v["engine_strongly_favoured_side_before"] = v["decided_before"].take();
            v.as_object_mut().unwrap().remove("decided_before");
            v["difficulty"] = crate::probes::difficulty(&a.turns[t.number - 1]);
            if let Some(p) = a.probes.moments.iter().find(|p| p.move_number == t.number) {
                let mut e = serde_json::to_value(p).unwrap();
                if let Some(o) = e["ownership_plan"].as_object_mut() {
                    o.remove("points");
                }
                // The full JSON retains per-ply probabilities/seeds. The report needs the complete
                // playable moves, evaluation provenance and origins, not duplicated sampler telemetry.
                if let Some(sequences) = e["sequences"].as_array_mut() {
                    for sequence in sequences {
                        if let Some(o) = sequence.as_object_mut() {
                            o.remove("probabilities");
                            o.remove("seed");
                        }
                    }
                }
                v["evidence"] = e;
            } else {
                v["evidence"] =
                    json!({"unavailable":{"deeper_search":"not selected for featured analysis"}});
            }
            v
        })
        .collect();
    let keep: std::collections::HashSet<_> = a
        .teaching
        .iter()
        .map(|t| t.number)
        .chain(a.praise.iter().map(|p| p.number))
        .collect();
    let mut details = serde_json::Map::new();
    for r in a.reviews.iter().filter(|r| keep.contains(&r.number)) {
        let best = r
            .alternatives
            .first()
            .map(|c| c.score_lead)
            .unwrap_or(r.score_before);
        let cs:Vec<_>=r.alternatives.iter().map(|c|json!({"move":c.mv,"loss_vs_best":crate::probes::sign(r.color)*(best-c.score_lead),"visits":c.visits,"pv":c.pv,"score_black":c.score_lead})).collect();
        details.insert(r.number.to_string(),json!({"number":r.number,"player":r.color.letter(),"move":r.mv,"point_loss":r.point_loss,"category":r.category.label(),"score_before":lead(r.score_before),"score_after":lead(r.score_after),"winrate_before":format!("{:.1}%",r.winrate_before*100.0),"winrate_after":format!("{:.1}%",r.winrate_after*100.0),"candidates":cs}));
    }
    let praise: Vec<_> = a
        .praise
        .iter()
        .map(|p| {
            let mut v = serde_json::to_value(p).unwrap();
            v["move_number"] = json!(p.number);
            v
        })
        .collect();
    let turning:Vec<_>=a.reviews.iter().filter(|r|(r.winrate_before-0.5)*(r.winrate_after-0.5)<0.0 || ((0.05..=0.95).contains(&r.winrate_before) && r.winrate_loss.abs()>=0.15)).map(|r|json!({"move_number":r.number,"color":r.color.letter(),"move":r.mv,"score_before":lead(r.score_before),"score_after":lead(r.score_after),"point_loss":r.point_loss})).collect();
    let mut accuracy = serde_json::Map::new();
    for c in [Color::Black, Color::White] {
        let rs: Vec<_> = a.reviews.iter().filter(|r| r.color == c).collect();
        let mean = if rs.is_empty() {
            0.0
        } else {
            rs.iter().map(|r| r.point_loss.max(0.0)).sum::<f64>() / rs.len() as f64
        };
        accuracy.insert(
            c.letter().into(),
            json!({"moves":rs.len(),"mean_loss":mean}),
        );
    }
    let mut data = json!({"report_format":5,"evidence_version":1,
        "game_info":{"board_size":g.size_x,"board_size_y":g.size_y,"black_player":g.player_black,"white_player":g.player_white,"student":student.letter(),"student_reason":a.student_reason,"num_moves":g.moves.len(),"komi":a.komi,"rules":a.rules,"handicap":g.handicap.unwrap_or(0),"setup_black":coords(&g.setup_black),"setup_white":coords(&g.setup_white),"initial_player":g.who_moves_first().letter(),"result":g.result,"date":g.date,"analysis_strength":a.visits_setting},
        "moves":moves,"move_details":details,"teaching":{"student":student.letter(),"candidates":candidates,"praise":praise},
        "summary":{"accuracy":accuracy,"turning_points":turning},"arc":{"phases":a.phases,"status_changes":a.status_changes},"openings":a.openings,"history":a.history,
        "endgame_values":a.probes.endgame_values,"missed_opportunities":a.probes.missed_opportunities,"rank_fit":a.probes.rank_fit,"profiles":a.probes.profiles,
        "provenance":{"engine":a.engine_version,"started_at":a.started_at,"elapsed_seconds":a.elapsed_seconds,"probe_queries":a.probes.queries,"probe_seconds":a.probes.elapsed_seconds,"warnings":a.engine_warnings.iter().chain(a.probes.warnings.iter()).collect::<Vec<_>>(),"sgf_warnings":g.warnings},
        "interpretation":{"scores":"Black-view; B+ / W+","deltas":"beneficiary explicitly named; positive loss is a cost","status_changes":"ownership predictions, not life/death proof","sequences":"from_turn is actual moves before branching; first_to_move explicit; each pass changes player","human":"sampled examples, not opponent-adjusted severity","rank_fit":"profile similarity, not a calibrated rank","phases":"move-count heuristic"}});
    round_numbers(&mut data);
    data
}
pub fn lead(score: f64) -> String {
    if score >= 0.0 {
        format!("B+{score:.1}")
    } else {
        format!("W+{:.1}", -score)
    }
}

fn round_numbers(v: &mut Value) {
    match v {
        Value::Array(a) => a.iter_mut().for_each(round_numbers),
        Value::Object(o) => {
            for (key, value) in o {
                if key.contains("prob") {
                    if let Some(f) = value.as_f64() {
                        *value = json!((f * 1e9).round() / 1e9);
                    }
                } else {
                    round_numbers(value);
                }
            }
        }
        Value::Number(n) if n.is_f64() => {
            if let Some(f) = n.as_f64() {
                *v = json!((f * 1000.0).round() / 1000.0);
            }
        }
        _ => {}
    }
}
