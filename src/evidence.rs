//! Compact, self-contained report data. Heatmap arrays stay in the full analysis JSON.
use crate::analysis::{Category, GameAnalysis, MoveReview};
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
        .map(|r| {
            // Extend the existing replay list; do not duplicate it as a separate timeline.
            // Scores stay Black-view even on White's moves. Loss belongs to the mover.
            let mut m = json!({"number":r.number,"color":r.color.letter(),"move":r.mv,
                "point_loss":r.point_loss,"score_black":r.score_after});
            if let Some(seconds) = valid_time(r) {
                m["time_spent_seconds"] = json!(seconds);
            }
            m
        })
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
    // Match the detailed report's supporting positions, without variations at every move.
    let mut opponents: Vec<_> = a
        .reviews
        .iter()
        .filter(|r| r.color != student && r.point_loss >= 2.0)
        .collect();
    opponents.sort_by(|x, y| y.point_loss.total_cmp(&x.point_loss));
    let opponent_moves: Vec<_> = opponents.iter().take(6).map(|r| r.number).collect();
    let checkpoint_interval = if g.moves.len() > 200 { 50 } else { 25 };
    let checkpoints: Vec<_> = a
        .reviews
        .iter()
        .filter(|r| r.number % checkpoint_interval == 0)
        .map(|r| r.number)
        .collect();
    let last_move = a.reviews.last().map(|r| r.number);
    let keep: std::collections::HashSet<_> = a
        .teaching
        .iter()
        .map(|t| t.number)
        .chain(a.praise.iter().map(|p| p.number))
        .chain(opponent_moves.iter().copied())
        .chain(checkpoints.iter().copied())
        .chain(last_move)
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
            json!({"moves":rs.len(),"mean_loss":mean,
                "total_loss":rs.iter().map(|r|r.point_loss.max(0.0)).sum::<f64>(),
                "median_loss":median(rs.iter().map(|r|r.point_loss.max(0.0)).collect()),
                "top1_moves":rs.iter().filter(|r|r.rank==Some(0)).count(),
                "top3_moves":rs.iter().filter(|r|r.rank.is_some_and(|k|k<3)).count(),
                "ranked_moves":rs.iter().filter(|r|r.rank.is_some()).count(),
                "category_counts":Category::all().iter().map(|cat|(cat.label().to_string(),json!(rs.iter().filter(|r|r.category==*cat).count()))).collect::<serde_json::Map<_,_>>() }),
        );
    }
    let initial = a.turns.iter().find(|t| t.turn == 0).map(|t|
        json!({"turn":0,"score_black":t.score_lead,"winrate_black":t.winrate,"visits":t.visits}));
    let mut data = json!({"report_format":5,"evidence_version":1,
        "game_info":{"board_size":g.size_x,"board_size_y":g.size_y,"black_player":g.player_black,"white_player":g.player_white,"student":student.letter(),"student_reason":a.student_reason,"num_moves":g.moves.len(),"komi":a.komi,"rules":a.rules,"handicap":g.handicap.unwrap_or(0),"setup_black":coords(&g.setup_black),"setup_white":coords(&g.setup_white),"initial_player":g.who_moves_first().letter(),"result":g.result,"date":g.date,"analysis_strength":a.visits_setting},
        "initial_position":initial,"moves":moves,"move_details":details,
        "context_moves":{"opponent_mistakes":opponent_moves,"checkpoints":checkpoints,"last_move":last_move},
        "teaching":{"student":student.letter(),"candidates":candidates,"praise":praise},
        "summary":{"accuracy":accuracy,"turning_points":turning,"timing":timing(a)},"arc":{"phases":a.phases,"status_changes":a.status_changes},"openings":a.openings,"history":a.history,
        "endgame_values":a.probes.endgame_values,"missed_opportunities":a.probes.missed_opportunities,"rank_fit":a.probes.rank_fit,"profiles":a.probes.profiles,
        "provenance":{"engine":a.engine_version,"started_at":a.started_at,"elapsed_seconds":a.elapsed_seconds,"probe_queries":a.probes.queries,"probe_seconds":a.probes.elapsed_seconds,"warnings":a.engine_warnings.iter().chain(a.probes.warnings.iter()).collect::<Vec<_>>(),"sgf_warnings":g.warnings},
        "interpretation":{"scores":"Black-view; B+ / W+","deltas":"beneficiary explicitly named; positive loss is a cost","status_changes":"ownership predictions, not life/death proof","sequences":"from_turn is actual moves before branching; first_to_move explicit; each pass changes player","human":"sampled examples, not opponent-adjusted severity","rank_fit":"profile similarity, not a calibrated rank","phases":"move-count heuristic",
            "moves":"score_black is the numeric Black lead AFTER this move; positive favours Black, negative White. Before move 1 use initial_position, otherwise the preceding move. point_loss belongs to the mover; negative values are search-estimate gains, not proof of superior play.",
            "accuracy":"Loss summaries clamp negative loss to zero; median averages the two middle values. top1/top3 are searched-choice counts, not percentages or rank estimates; ranked_moves gives coverage. Total loss is not final margin.",
            "timing":"Optional seconds derived from SGF clocks. Split at each player's own median, ties in at_or_below_median; missing/empty is unknown, not zero. Association is not causation.",
            "status_coverage":"Only detected significant group events are retained; this is not a complete capture ledger or a safety assessment of every group."}});
    round_numbers(&mut data);
    data
}

fn median(mut values: Vec<f64>) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    values.sort_by(f64::total_cmp);
    let mid = values.len() / 2;
    Some(if values.len() % 2 == 0 {
        (values[mid - 1] + values[mid]) / 2.0
    } else {
        values[mid]
    })
}

fn valid_time(r: &MoveReview) -> Option<f64> {
    r.time_spent.filter(|t| t.is_finite() && *t >= 0.0)
}

fn timing(a: &GameAnalysis) -> Value {
    let mut players = serde_json::Map::new();
    for color in [Color::Black, Color::White] {
        let rs: Vec<_> = a.reviews.iter().filter(|r| r.color == color).collect();
        let timed: Vec<_> = rs
            .iter()
            .filter_map(|r| valid_time(r).map(|s| (*r, s)))
            .collect();
        let split = median(timed.iter().map(|(_, s)| *s).collect());
        let bucket = |below: bool| {
            let losses: Vec<_> = timed
                .iter()
                .filter(|(_, s)| (*s <= split.unwrap()) == below)
                .map(|(r, _)| r.point_loss.max(0.0))
                .collect();
            json!({"moves":losses.len(),"mean_loss":if losses.is_empty() {None} else {Some(losses.iter().sum::<f64>()/losses.len() as f64)}})
        };
        let mut value = json!({"timed_moves":timed.len(),"untimed_moves":rs.len()-timed.len()});
        if let Some(split) = split {
            value["median_seconds"] = json!(split);
            value["mean_seconds"] =
                json!(timed.iter().map(|(_, s)| s).sum::<f64>() / timed.len() as f64);
            value["at_or_below_median"] = bucket(true);
            value["above_median"] = bucket(false);
        }
        players.insert(color.letter().to_string(), value);
    }
    json!({"status":if a.reviews.iter().any(|r|valid_time(r).is_some()) {"available"} else {"unavailable"},"players":players})
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

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> GameAnalysis {
        let game =
            crate::sgf::parse_game("(;SZ[9]HA[2]AB[cc][gg]PL[W];W[dc];B[cd];W[];B[ee])").unwrap();
        let scores = [5.0, 9.0, 3.0, 2.0, 2.4];
        let losses = [4.0, 6.0, -1.0, -0.4];
        let times = [2.0, 100.0, 10.0, 300.0];
        let ranks = [Some(2), None, Some(0), Some(1)];
        let reviews: Vec<Value> = (0..4)
            .map(|i| {
                json!({
                    "number":i+1,"color":if i%2==0 {Color::White} else {Color::Black},
                    "mv":(["D7","C6","pass","E5"][i]),"comment":null,
                    "winrate_before":0.9,"winrate_after":0.9,
                    "score_before":scores[i],"score_after":scores[i+1],
                    "point_loss":losses[i],"winrate_loss":0.0,
                    "category":Category::from_loss(losses[i]),"rank":ranks[i],
                    "played_candidate":null,"alternatives":[],"time_spent":times[i]
                })
            })
            .collect();
        let turns: Vec<Value> = scores
            .iter()
            .enumerate()
            .map(|(i, s)| {
                json!({
                    "turn":i,"to_move":if i%2==0 {Color::White} else {Color::Black},
                    "winrate":0.9,"score_lead":s,"score_stdev":null,"visits":150,"candidates":[]
                })
            })
            .collect();
        serde_json::from_value(json!({"game":game,"rules":"japanese","komi":6.5,
            "options":crate::analysis::AnalysisOptions::default(),"engine_version":"fixture",
            "visits_setting":"150 visits","started_at":"test","elapsed_seconds":0.0,
            "reviews":reviews,"turns":turns,"engine_warnings":[],"student":Color::White
        }))
        .unwrap()
    }

    #[test]
    fn timeline_keeps_black_perspective_on_white_moves_and_passes() {
        let a = fixture();
        let d = build(&a);
        assert_eq!(d["game_info"]["student"], "W");
        assert_eq!(d["game_info"]["initial_player"], "W");
        assert_eq!(d["game_info"]["setup_black"], json!(["C7", "G3"]));
        assert_eq!(d["initial_position"]["score_black"], 5.0);
        assert_eq!(d["moves"][0]["color"], "W");
        assert_eq!(d["moves"][0]["score_black"], 9.0); // White lost 4 points.
        assert_eq!(d["moves"][0]["point_loss"], 4.0);
        assert_eq!(d["moves"][2]["move"], "pass");
        assert_eq!(d["moves"][2]["point_loss"], -1.0); // Preserve search disagreement.
        assert_eq!(d["moves"][3]["score_black"], 2.4);
        let white = &d["summary"]["accuracy"]["W"];
        assert_eq!(white["mean_loss"], 2.0);
        assert_eq!(white["median_loss"], 2.0); // Even-sized median, negatives clamped.
        assert_eq!(white["total_loss"], 4.0);
        assert_eq!(white["top1_moves"], 1);
        assert_eq!(white["top3_moves"], 2);
        assert_eq!(d["summary"]["accuracy"]["B"]["ranked_moves"], 1);
        assert_eq!(white["category_counts"]["mistake"], 1);
        assert_eq!(d["context_moves"]["opponent_mistakes"], json!([2]));
        assert_eq!(d["context_moves"]["last_move"], 4);
        assert!(d["move_details"].get("2").is_some());
        assert!(d["move_details"].get("4").is_some());
        assert!(d["move_details"].get("1").is_none());
    }

    #[test]
    fn timing_uses_each_players_clocks_and_reports_empty_buckets() {
        let mut a = fixture();
        let d = build(&a);
        let timing = &d["summary"]["timing"]["players"];
        assert_eq!(timing["W"]["median_seconds"], 6.0);
        assert_eq!(timing["B"]["median_seconds"], 200.0);
        assert_eq!(timing["W"]["at_or_below_median"]["mean_loss"], 4.0);
        assert_eq!(timing["B"]["at_or_below_median"]["mean_loss"], 6.0);
        for r in &mut a.reviews {
            r.time_spent = Some(0.0);
        }
        let d = build(&a);
        assert_eq!(d["moves"][0]["time_spent_seconds"], 0.0);
        assert_eq!(
            d["summary"]["timing"]["players"]["W"]["at_or_below_median"]["moves"],
            2
        );
        assert_eq!(
            d["summary"]["timing"]["players"]["W"]["above_median"]["moves"],
            0
        );
        assert!(d["summary"]["timing"]["players"]["W"]["above_median"]["mean_loss"].is_null());
    }

    #[test]
    fn absent_timing_and_initial_evaluation_are_not_zero() {
        let mut a = fixture();
        for r in &mut a.reviews {
            r.time_spent = None;
        }
        a.reviews[0].time_spent = Some(-1.0);
        a.reviews[1].time_spent = Some(f64::NAN);
        a.turns.remove(0);
        let d = build(&a);
        assert!(d["initial_position"].is_null());
        assert_eq!(d["summary"]["timing"]["status"], "unavailable");
        assert_eq!(d["summary"]["timing"]["players"]["W"]["untimed_moves"], 2);
        assert!(d["summary"]["timing"]["players"]["W"]
            .get("median_seconds")
            .is_none());
        assert!(d["moves"]
            .as_array()
            .unwrap()
            .iter()
            .all(|m| m.get("time_spent_seconds").is_none()));
        a.reviews.clear();
        a.turns.clear();
        a.game.moves.clear();
        let empty = build(&a);
        assert_eq!(empty["summary"]["accuracy"]["B"]["moves"], 0);
        assert!(empty["summary"]["accuracy"]["B"]["median_loss"].is_null());
    }

    #[test]
    fn supporting_entries_are_selected_once_at_checkpoint_intervals() {
        for (length, interval) in [(100, 25), (250, 50)] {
            let mut a = fixture();
            let review = a.reviews[0].clone();
            let mv = a.game.moves[0].clone();
            a.game.moves = vec![mv; length];
            a.reviews = (1..=length)
                .map(|number| {
                    let mut r = review.clone();
                    r.number = number;
                    r.color = if number % 2 == 0 {
                        Color::Black
                    } else {
                        Color::White
                    };
                    r.point_loss = number as f64;
                    r
                })
                .collect();
            let d = build(&a);
            let expected_checkpoints: Vec<_> = (interval..=length).step_by(interval).collect();
            assert_eq!(
                d["context_moves"]["checkpoints"],
                json!(expected_checkpoints)
            );
            assert_eq!(
                d["context_moves"]["opponent_mistakes"]
                    .as_array()
                    .unwrap()
                    .len(),
                6
            );
            assert_eq!(d["context_moves"]["last_move"], length);
            // Last move is also an opponent mistake and a checkpoint; it appears once.
            assert_eq!(
                d["move_details"].as_object().unwrap().len(),
                expected_checkpoints.len() + 5
            );
        }
    }
}
