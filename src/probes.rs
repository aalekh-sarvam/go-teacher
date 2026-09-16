//! Finite, cancellable teaching searches using the existing engine and real game history.
use crate::analysis::{run_query_ext, AnalysisOptions, GameAnalysis, QueryExtras, TurnEval};
use crate::board::LegalPosition;
use crate::katago::Engine;
use crate::sgf::{Color, Coord};
use anyhow::{anyhow, bail, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap};
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ProbeOptions {
    pub enabled: bool,
    pub featured: usize,
    pub visits: u64,
    pub rollout_plies: usize,
    pub opponent_profile: Option<String>,
}
impl Default for ProbeOptions {
    fn default() -> Self {
        Self {
            enabled: true,
            featured: 4,
            visits: 300,
            rollout_plies: 24,
            opponent_profile: None,
        }
    }
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProbeAnalysis {
    pub moments: Vec<Moment>,
    pub difficulty: Vec<Value>,
    pub missed_opportunities: Vec<Value>,
    pub endgame_values: Vec<Value>,
    pub rank_fit: Value,
    pub profiles: Value,
    pub queries: usize,
    pub cache_hits: usize,
    pub elapsed_seconds: f64,
    pub seconds_by_kind: BTreeMap<String, f64>,
    pub warnings: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sequence {
    pub id: String,
    pub from_turn: usize,
    pub first_to_move: Color,
    pub moves: Vec<String>,
    pub source: String,
    pub profile: Option<String>,
    pub probabilities: Vec<f32>,
    pub seed: Option<String>,
    pub score_black: Option<f64>,
    pub visits: u64,
    pub stop_reason: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Moment {
    pub move_number: usize,
    pub mover: Color,
    pub pass_comparison: Value,
    pub move_values: Vec<Value>,
    pub initiative: Vec<Value>,
    pub ownership_plan: Value,
    pub local_reading: Value,
    pub tree: Vec<Value>,
    pub sequences: Vec<Sequence>,
    pub human_refutations: Vec<Value>,
    pub rollout_comparisons: Vec<Value>,
    pub unavailable: BTreeMap<String, String>,
}
pub fn sign(c: Color) -> f64 {
    if c == Color::Black {
        1.0
    } else {
        -1.0
    }
}
fn round(v: f64) -> f64 {
    (v * 1000.0).round() / 1000.0
}
fn index(mv: &str, sx: usize, sy: usize) -> Option<usize> {
    if mv == "pass" {
        return Some(sx * sy);
    }
    Coord::from_gtp(mv, sy)
        .filter(|c| c.x < sx)
        .map(|c| c.y * sx + c.x)
}
fn coordinate(i: usize, sx: usize, sy: usize) -> String {
    if i == sx * sy {
        "pass".into()
    } else {
        Coord {
            x: i % sx,
            y: i / sx,
        }
        .to_gtp(sy)
    }
}
fn entropy(p: &Option<Vec<f32>>) -> Option<f64> {
    let p = p.as_ref()?;
    let mass: f64 = p.iter().filter(|&&v| v > 0.0).map(|&v| v as f64).sum();
    if mass <= 0.0 {
        return None;
    }
    Some(round(
        p.iter()
            .filter(|&&v| v > 0.0)
            .map(|&v| {
                let q = v as f64 / mass;
                -q * q.ln()
            })
            .sum(),
    ))
}
pub fn difficulty(t: &TurnEval) -> Value {
    let Some(best) = t.candidates.first() else {
        return json!({"status":"unavailable","reason":"no candidates"});
    };
    let cs: Vec<_> = t
        .candidates
        .iter()
        .filter(|c| c.visits >= 10.max(t.visits / 50))
        .collect();
    let good = cs
        .iter()
        .filter(|c| sign(t.to_move) * (best.score_lead - c.score_lead) <= 1.0)
        .count();
    let gap = cs
        .get(1)
        .map(|c| round(sign(t.to_move) * (best.score_lead - c.score_lead)));
    // One clear move: the top candidate took almost every visit and the next searched move is
    // clearly worse. Under-searched: the whole position had few visits. These are different things.
    let top_share = if t.visits > 0 { best.visits as f64 / t.visits as f64 } else { 0.0 };
    let label = if t.visits < 100 {
        "under-searched"
    } else if top_share >= 0.9 && gap.map_or(true, |g| g >= 2.0) {
        "one clear move"
    } else if cs.len() < 2 {
        "one clear move"
    } else if good == 1 {
        "narrow choice in searched moves"
    } else {
        "several searched choices"
    };
    json!({"turn":t.turn,"near_best_searched":good,"candidates_tested":cs.len(),"gap_points_for_mover":gap,
        "top_visit_share":round(top_share),
        "policy_entropy_nats":entropy(&t.policy),"human_entropy_nats":entropy(&t.human_policy),
        "label":label,
        "visits":t.visits,"limited_search":t.visits<100,"caveat":"Search coverage and policy entropy are difficulty signals, not a human difficulty measurement."})
}

struct Runner<'a, F> {
    engine: &'a Engine,
    a: &'a GameAnalysis,
    cancel: &'a CancellationToken,
    progress: &'a mut F,
    done: usize,
    total: usize,
    queries: usize,
    cache_hits: usize,
    cache: HashMap<String, TurnEval>,
    warnings: Vec<String>,
    timing: BTreeMap<String, f64>,
}
impl<F: FnMut(usize, usize, Option<&TurnEval>, &str)> Runner<'_, F> {
    async fn eval(
        &mut self,
        kind: &str,
        prefix: usize,
        line: &[String],
        visits: u64,
        profile: Option<&str>,
        allowed: Option<Vec<String>>,
    ) -> Result<TurnEval> {
        if self.cancel.is_cancelled() {
            bail!("analysis cancelled");
        }
        let mut game = self.a.game.clone();
        game.moves.truncate(prefix);
        // Keep the original next player when truncation removes an explicit PL/non-alternating move.
        let mut player = self.a.turns[prefix].to_move;
        if prefix == 0 {
            game.first_player = Some(player);
        }
        let mut extras = QueryExtras {
            include_moves_ownership: visits > 1,
            priority: 5,
            timeout_seconds: Some(120),
            human_profile: profile.map(str::to_string),
            allow_moves: allowed,
            allow_depth: 12,
            ..Default::default()
        };
        let mut legal = LegalPosition::from_game(&self.a.game, prefix, &self.a.rules)?;
        legal.to_move = player;
        for mv in line {
            legal.play(mv)?;
            extras.extra_moves.push((
                player,
                if mv == "pass" {
                    None
                } else {
                    Coord::from_gtp(mv, game.size_y)
                },
            ));
            player = player.opponent();
        }
        let turn = prefix + line.len();
        let key = serde_json::to_string(&crate::analysis::build_query_ext(
            &game,
            &self.a.rules,
            self.a.komi,
            &self.a.options,
            &[turn],
            Some(visits),
            &extras,
        ))?;
        if let Some(t) = self.cache.get(&key) {
            self.cache_hits += 1;
            return Ok(t.clone());
        }
        self.queries += 1;
        (self.progress)(
            self.done,
            self.total,
            None,
            &format!(
                "teaching evidence: {kind}, move {} ({} queries)",
                prefix + 1,
                self.queries
            ),
        );
        let started = std::time::Instant::now();
        let result = run_query_ext(
            self.engine,
            &game,
            &self.a.rules,
            self.a.komi,
            &self.a.options,
            &[turn],
            Some(visits),
            &extras,
            self.cancel,
            &mut self.warnings,
            |_| {},
        )
        .await;
        *self.timing.entry(kind.into()).or_default() += started.elapsed().as_secs_f64();
        let t = result?
            .into_iter()
            .next()
            .ok_or_else(|| anyhow!("no probe result"))?;
        self.cache.insert(key, t.clone());
        Ok(t)
    }
    fn unavailable(&mut self, m: &mut Moment, key: &str, e: impl std::fmt::Display) {
        m.unavailable.insert(key.into(), e.to_string());
    }
    fn sequence(
        &self,
        id: &str,
        prefix: usize,
        moves: Vec<String>,
        source: &str,
        t: Option<&TurnEval>,
    ) -> Sequence {
        let mut p = LegalPosition::from_game(&self.a.game, prefix, &self.a.rules).ok();
        if let Some(p) = p.as_mut() {
            p.to_move = self.a.turns[prefix].to_move;
        }
        let mut valid = Vec::new();
        let mut stop = "line end".to_string();
        for mv in moves {
            match p
                .as_mut()
                .ok_or_else(|| anyhow!("invalid game prefix"))
                .and_then(|p| p.play(&mv))
            {
                Ok(()) => valid.push(mv),
                Err(e) => {
                    stop = format!("truncated: {e}");
                    break;
                }
            }
            if valid.len() >= 2 && valid[valid.len() - 2..].iter().all(|m| m == "pass") {
                stop = "two passes".into();
                break;
            }
        }
        Sequence {
            id: id.into(),
            from_turn: prefix,
            first_to_move: self.a.turns[prefix].to_move,
            moves: valid,
            source: source.into(),
            profile: None,
            probabilities: vec![],
            seed: None,
            score_black: t.map(|t| round(t.score_lead)),
            visits: t.map(|t| t.visits).unwrap_or(0),
            stop_reason: stop,
        }
    }
    async fn rollout(
        &mut self,
        prefix: usize,
        initial: &str,
        level: &str,
        profile: &str,
        opponent: &str,
        plies: usize,
    ) -> Result<Sequence> {
        let seed = fnv(&format!(
            "{}|{prefix}|{initial}|{profile}|{opponent}",
            serde_json::to_string(&self.a.game)?
        ));
        let mut rng = seed;
        let mut line = vec![initial.to_string()];
        let mut probs = vec![];
        let mut stop = "ply limit".to_string();
        for _ in 1..plies {
            if line.len() >= 2 && line[line.len() - 2..].iter().all(|m| m == "pass") {
                stop = "two passes".into();
                break;
            }
            let player = if line.len() % 2 == 0 {
                self.a.turns[prefix].to_move
            } else {
                self.a.turns[prefix].to_move.opponent()
            };
            let selected_profile = if player == self.a.student.unwrap_or(Color::Black) {
                profile
            } else {
                opponent
            };
            let t = self
                .eval(
                    "human what-if",
                    prefix,
                    &line,
                    1,
                    Some(selected_profile),
                    None,
                )
                .await?;
            let policy = t
                .human_policy
                .as_ref()
                .ok_or_else(|| anyhow!("human policy was not returned"))?;
            let (mv, p) = sample(policy, self.a.game.size_x, self.a.game.size_y, &mut rng)
                .ok_or_else(|| anyhow!("no legal human-policy mass"))?;
            line.push(mv);
            probs.push(p);
        }
        let final_t = self
            .eval(
                "what-if endpoint",
                prefix,
                &line,
                self.a.options.probes.visits,
                None,
                None,
            )
            .await?;
        let mut seq = self.sequence(
            &format!("{initial}_{level}"),
            prefix,
            line,
            "sampled human policy; strong-engine endpoint",
            Some(&final_t),
        );
        seq.profile = Some(format!("student {profile}; opponent {opponent}"));
        seq.probabilities = probs;
        seq.seed = Some(seed.to_string());
        seq.stop_reason = stop;
        Ok(seq)
    }
}
fn fnv(s: &str) -> u64 {
    s.bytes().fold(14695981039346656037, |h, b| {
        (h ^ b as u64).wrapping_mul(1099511628211)
    })
}
fn sample(p: &[f32], sx: usize, sy: usize, rng: &mut u64) -> Option<(String, f32)> {
    if p.len() != sx * sy + 1 {
        return None;
    }
    *rng = rng.wrapping_add(0x9e3779b97f4a7c15);
    let mut z = *rng;
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
    z ^= z >> 31;
    let mass: f64 = p
        .iter()
        .filter(|&&v| v > 0.0 && v.is_finite())
        .map(|&v| v as f64)
        .sum();
    if mass <= 0.0 {
        return None;
    }
    let mut u = (z >> 11) as f64 / (1u64 << 53) as f64 * mass;
    for (i, &v) in p.iter().enumerate() {
        if v > 0.0 && v.is_finite() {
            u -= v as f64;
            if u <= 0.0 {
                return Some((coordinate(i, sx, sy), v));
            }
        }
    }
    None
}
fn nearby(a: &str, b: &str, sx: usize, sy: usize) -> bool {
    match (Coord::from_gtp(a, sy), Coord::from_gtp(b, sy)) {
        (Some(a), Some(b)) => {
            a.x.abs_diff(b.x).max(a.y.abs_diff(b.y))
                <= if sx <= 9 {
                    2
                } else if sx <= 13 {
                    3
                } else {
                    4
                }
        }
        _ => false,
    }
}
fn initiative(mv: &str, root: &TurnEval, reply: &TurnEval, sx: usize, sy: usize) -> Value {
    let Some(best) = reply.candidates.first() else {
        return json!({"move":mv,"class":"unclear"});
    };
    let local = nearby(mv, &best.mv, sx, sy);
    let tenuki = reply
        .candidates
        .iter()
        .filter(|c| c.visits >= 10 && !nearby(mv, &c.mv, sx, sy))
        .max_by(|a, b| {
            (sign(reply.to_move) * a.score_lead).total_cmp(&(sign(reply.to_move) * b.score_lead))
        });
    let cost = tenuki.map(|c| round(sign(reply.to_move) * (best.score_lead - c.score_lead)));
    let loss = sign(root.to_move) * (root.score_lead - best.score_lead);
    let class = if mv == "pass" || best.visits < 20 {
        "unclear"
    } else if local && loss > 1.5 {
        "local punishment"
    } else if !local {
        "reply elsewhere (gote-like)"
    } else if cost.is_some_and(|v| v >= 1.5) {
        "sente-like exchange"
    } else {
        "local reply; forcing status unclear"
    };
    json!({"move":mv,"class":class,"reply":best.mv,"reply_local":local,"ignoring_cost_for_opponent":cost,"exchange_loss_for_mover":round(loss),"visits":reply.visits,"source":"finite unrestricted search","caveat":"Locality and search value do not prove a forcing threat."})
}
fn pass_values(
    root: &TurnEval,
    after: &TurnEval,
    pass: &TurnEval,
    played: &str,
) -> (Value, Vec<Value>) {
    let s = sign(root.to_move);
    let comparison = json!({"beneficiary":root.to_move,"best_vs_pass_points":round(s*(root.score_lead-pass.score_lead)),"pass_score_black":round(pass.score_lead),"reply":pass.candidates.first().map(|c|&c.mv),"visits":pass.visits,"meaning":"whole-board gain against passing; not a local miai/deiri value"});
    let mut values:Vec<Value>=root.candidates.iter().take(4).map(|c|json!({"move":c.mv,"gain_vs_pass":round(s*(c.score_lead-pass.score_lead)),"visits":c.visits,"source":"candidate search","played":c.mv==played})).collect();
    values.retain(|v| v["move"] != played);
    values.push(json!({"move":played,"gain_vs_pass":round(s*(after.score_lead-pass.score_lead)),"visits":after.visits,"source":"actual after-position","played":true}));
    (comparison, values)
}
fn plan(
    root: &TurnEval,
    after: &TurnEval,
    played: &str,
    sx: usize,
    sy: usize,
    board: &crate::board::Board,
) -> Value {
    let Some(best) = root.candidates.first() else {
        return Value::Null;
    };
    let Some(ob) = &best.ownership else {
        return Value::Null;
    };
    let candidate = root
        .candidates
        .iter()
        .find(|c| c.mv == played && c.visits >= 100)
        .and_then(|c| c.ownership.as_ref());
    let Some(op) = candidate.or(after.ownership.as_ref()) else {
        return Value::Null;
    };
    if ob.len() != sx * sy || op.len() != sx * sy {
        return Value::Null;
    }
    let mut regions: BTreeMap<&str, f64> = BTreeMap::new();
    let mut points = Vec::new();
    for i in 0..ob.len() {
        let d = sign(root.to_move) * (ob[i] - op[i]) as f64;
        let c = Coord {
            x: i % sx,
            y: i / sx,
        };
        *regions
            .entry(crate::teaching::region_name(c, sx, sy))
            .or_default() += d;
        if d.abs() >= 0.3 {
            points.push(json!({"point":c.to_gtp(sy),"cost_of_played":round(d)}));
        }
    }
    points.sort_by(|a, b| {
        b["cost_of_played"]
            .as_f64()
            .unwrap()
            .abs()
            .total_cmp(&a["cost_of_played"].as_f64().unwrap().abs())
    });
    let total: f64 = regions.values().sum();
    let groups:Vec<_>=board.groups().into_iter().filter(|(_,stones,_)|stones.len()>=2).filter_map(|(color,stones,liberties)|{
        let own_best=sign(color)*stones.iter().map(|c|ob[c.y*sx+c.x] as f64).sum::<f64>()/stones.len() as f64;
        let own_played=sign(color)*stones.iter().map(|c|op[c.y*sx+c.x] as f64).sum::<f64>()/stones.len() as f64;
        if (own_best-own_played).abs()<0.5{return None;}
        Some(json!({"color":color,"stones":stones.iter().map(|c|c.to_gtp(sy)).collect::<Vec<_>>(),"liberties":liberties,"ownership_for_group_best":round(own_best),"ownership_for_group_played":round(own_played)}))
    }).collect();
    json!({"best":best.mv,"played":played,"beneficiary":root.to_move,"source_played":if candidate.is_some(){"candidate ownership"}else{"after-position ownership"},
        "regions":regions.into_iter().map(|(region,v)|json!({"region":region,"cost_of_played":round(v)})).collect::<Vec<_>>(),"points":points,"groups":groups,"net_ownership_change":round(total),
        "disagrees_with_score":(total-sign(root.to_move)*(root.score_lead-after.score_lead)).abs()>3.0,
        "caveat":"Predicted ownership change; not exact territory or proof of life/death. Positive favours the better move for the mover."})
}

fn rank_profile(rank: Option<&str>) -> Option<String> {
    let r = rank?.trim().to_ascii_lowercase().replace(' ', "");
    let r = r.strip_prefix("rank_").unwrap_or(&r);
    let num = r
        .strip_suffix('k')
        .or_else(|| r.strip_suffix('d'))?
        .parse::<usize>()
        .ok()?;
    if (r.ends_with('k') && (1..=20).contains(&num)) || (r.ends_with('d') && (1..=9).contains(&num))
    {
        Some(format!("rank_{r}"))
    } else {
        None
    }
}
fn stronger(profile: &str) -> Option<String> {
    let r = profile.strip_prefix("rank_")?;
    let num = r[..r.len().checked_sub(1)?].parse::<i32>().ok()?;
    let rank = if r.ends_with('k') {
        -num
    } else if r.ends_with('d') {
        num - 1
    } else {
        return None;
    };
    let target = (rank + 2).min(8);
    Some(if target < 0 {
        format!("rank_{}k", -target)
    } else {
        format!("rank_{}d", target + 1)
    })
}
async fn local_reading<F: FnMut(usize, usize, Option<&TurnEval>, &str)>(
    run: &mut Runner<'_, F>,
    i: usize,
    focus: &str,
) -> Result<Value> {
    let p = LegalPosition::from_game(&run.a.game, i, &run.a.rules)?;
    let anchor = Coord::from_gtp(focus, run.a.game.size_y);
    let mut groups = p.board.groups();
    groups.retain(|(_, stones, libs)| *libs <= 4 && (stones.len() >= 2 || *libs == 1));
    groups.sort_by_key(|(_, stones, libs)| {
        (
            *libs,
            anchor
                .map(|a| {
                    stones
                        .iter()
                        .map(|s| s.x.abs_diff(a.x) + s.y.abs_diff(a.y))
                        .min()
                        .unwrap_or(99)
                })
                .unwrap_or(99),
        )
    });
    let Some((color, stones, liberties)) = groups.first() else {
        return Ok(
            json!({"status":"not applicable","reason":"no nearby low-liberty group selected"}),
        );
    };
    let sx = run.a.game.size_x;
    let sy = run.a.game.size_y;
    let x0 = stones.iter().map(|c| c.x).min().unwrap().saturating_sub(2);
    let x1 = (stones.iter().map(|c| c.x).max().unwrap() + 2).min(sx - 1);
    let y0 = stones.iter().map(|c| c.y).min().unwrap().saturating_sub(2);
    let y1 = (stones.iter().map(|c| c.y).max().unwrap() + 2).min(sy - 1);
    let allowed: Vec<String> = (y0..=y1)
        .flat_map(|y| (x0..=x1).map(move |x| Coord { x, y }.to_gtp(sy)))
        .chain(std::iter::once("pass".into()))
        .collect();
    let mut trials = Vec::new();
    for first in [*color, color.opponent()] {
        let extra = if run.a.turns[i].to_move == first {
            vec![]
        } else {
            vec!["pass".into()]
        };
        if !extra.is_empty() && i > 0 && run.a.game.moves[i - 1].point.is_none() {
            trials.push(json!({"first":first,"status":"unavailable","reason":"would create two consecutive passes"}));
            continue;
        }
        let t = run
            .eval(
                "restricted local reading",
                i,
                &extra,
                run.a.options.probes.visits,
                None,
                Some(allowed.clone()),
            )
            .await?;
        let best = t.candidates.first();
        let own = best
            .and_then(|c| c.ownership.as_ref())
            .or(t.ownership.as_ref());
        let mean = own.filter(|o| o.len() == sx * sy).map(|o| {
            sign(*color) * stones.iter().map(|c| o[c.y * sx + c.x] as f64).sum::<f64>()
                / stones.len() as f64
        });
        let line = best.map(|c| c.pv.clone()).unwrap_or_default();
        let mut full = extra.clone();
        full.extend(line);
        let seq = run.sequence(
            if first == *color {
                "defender_first"
            } else {
                "attacker_first"
            },
            i,
            full,
            "restricted local search",
            None,
        );
        trials.push(json!({"first":first,"move":best.map(|c|&c.mv),"group_ownership_for_defender":mean.map(round),"prediction":match mean {Some(v) if v>0.4=>"favourable",Some(v) if v< -0.4=>"unfavourable",_=>"unclear"},"visits":t.visits,"sequence":seq}));
    }
    Ok(
        json!({"status":"computed","group_color":color,"stones":stones.iter().map(|c|c.to_gtp(sy)).collect::<Vec<_>>(),"liberties":liberties,"allowed_moves":allowed,"restricted_plies":12,"trials":trials,
        "caveat":"Finite search restricted to a bounding box plus pass. Ownership is a prediction, not a life/death proof; outside moves, ko threats, seki and distant ladders may change the result."}),
    )
}

async fn rank_fit<F: FnMut(usize, usize, Option<&TurnEval>, &str)>(
    run: &mut Runner<'_, F>,
    profiles: &[String],
) -> Result<Value> {
    if run.engine.config.human_model.is_none() {
        return Ok(json!({"status":"unavailable","reason":"human model not loaded"}));
    }
    let student = run.a.student.unwrap_or(Color::Black);
    // Every non-pass student move, stride-sampled only when the game is long (cap 60 positions):
    // one network evaluation per position per profile, no search, so this stays cheap.
    let all: Vec<usize> = run.a.reviews.iter().filter(|r| r.color == student && r.mv != "pass").map(|r| r.number - 1).collect();
    let stride = all.len().div_ceil(60).max(1);
    let selected: Vec<usize> = all.into_iter().step_by(stride).collect();
    if selected.len() < 8 {
        return Ok(
            json!({"status":"unavailable","reason":"fewer than eight non-pass student moves","sample_count":selected.len()}),
        );
    }
    let mut probabilities: BTreeMap<String, Vec<(usize, f64)>> = BTreeMap::new();
    // move index -> list of (profile, prob of played move, played move is that profile's top move)
    let mut per_move: BTreeMap<usize, Vec<(String, f64, bool)>> = BTreeMap::new();
    for profile in profiles {
        let mut opts: AnalysisOptions = run.a.options.clone();
        opts.human_profile = Some(profile.clone());
        run.queries += 1;
        (run.progress)(
            run.done,
            run.total,
            None,
            &format!(
                "human-policy fit: {profile} ({} sampled moves)",
                selected.len()
            ),
        );
        let start = std::time::Instant::now();
        let extra = QueryExtras {
            priority: 5,
            timeout_seconds: Some(120),
            ..Default::default()
        };
        let turns = run_query_ext(
            run.engine,
            &run.a.game,
            &run.a.rules,
            run.a.komi,
            &opts,
            &selected,
            Some(1),
            &extra,
            run.cancel,
            &mut run.warnings,
            |_| {},
        )
        .await?;
        *run.timing.entry("rank policy fit".into()).or_default() += start.elapsed().as_secs_f64();
        for t in turns {
            let r = &run.a.reviews[t.turn];
            let idx = index(&r.mv, run.a.game.size_x, run.a.game.size_y);
            if let (Some(pol), Some(i)) = (t.human_policy.as_ref(), idx) {
                if let Some(p) = pol.get(i) {
                    if *p >= 0.0 {
                        probabilities.entry(profile.clone()).or_default().push((t.turn, (*p as f64).max(1e-6)));
                        let top = pol.iter().cloned().fold(f32::MIN, f32::max);
                        per_move.entry(t.turn).or_default().push((profile.clone(), *p as f64, (*p - top).abs() < 1e-9));
                    }
                }
            }
        }
    }
    // Likelihood-weighted rank estimate over the tested ladder, tempered so ~8 moves carry one unit
    // of evidence; the band is one weighted standard deviation. Descriptive similarity, not a rating.
    let estimate_for = |vs: &dyn Fn(&str) -> Option<(f64, usize)>| -> Option<Value> {
        let mut pts: Vec<(f64, f64, usize)> = Vec::new(); // (rank value, total log-lik, n)
        for p in profiles {
            if let (Some(v), Some((ll, n))) = (rank_value(p), vs(p)) {
                pts.push((v, ll, n));
            }
        }
        if pts.len() < 3 {
            return None;
        }
        let n = pts[0].2;
        if n < 8 {
            return None;
        }
        let temp = (n as f64 / 8.0).max(1.0);
        let max_ll = pts.iter().map(|x| x.1).fold(f64::MIN, f64::max);
        let weights: Vec<f64> = pts.iter().map(|x| ((x.1 - max_ll) / temp).exp()).collect();
        let wsum: f64 = weights.iter().sum();
        let mean = pts.iter().zip(&weights).map(|(x, w)| x.0 * w).sum::<f64>() / wsum;
        let var = pts.iter().zip(&weights).map(|(x, w)| w * (x.0 - mean).powi(2)).sum::<f64>() / wsum;
        let sd = var.sqrt().max(1.0);
        Some(json!({
            "rank_value": round(mean), "rank_label": rank_label(mean),
            "low_label": rank_label(mean - sd), "high_label": rank_label(mean + sd),
            "band_stones": round(sd), "samples": n,
            "wording": format!("plays like a {} in this game (range {}–{}, {} moves)", rank_label(mean), rank_label(mean - sd), rank_label(mean + sd), n),
        }))
    };
    let mut fits = Vec::new();
    for phase in ["whole game", "opening", "middlegame", "endgame"] {
        let mut scores = Vec::new();
        for (profile, values) in &probabilities {
            let vs: Vec<_> = values
                .iter()
                .filter(|(i, _)| {
                    phase == "whole game"
                        || crate::teaching::phase_of(i + 1, run.a.reviews.len()) == phase
                })
                .collect();
            if !vs.is_empty() {
                scores.push(json!({"profile":profile,"mean_log_likelihood":round(vs.iter().map(|(_,p)|p.ln()).sum::<f64>()/vs.len() as f64),"sample_count":vs.len()}));
            }
        }
        scores.sort_by(|a, b| {
            b["mean_log_likelihood"]
                .as_f64()
                .unwrap()
                .total_cmp(&a["mean_log_likelihood"].as_f64().unwrap())
        });
        let n = scores
            .first()
            .and_then(|s| s["sample_count"].as_u64())
            .unwrap_or(0);
        let winner = if n >= 8 {
            scores.first().map(|s| s["profile"].clone())
        } else {
            None
        };
        let estimate = estimate_for(&|profile: &str| {
            probabilities.get(profile).map(|values| {
                let vs: Vec<f64> = values
                    .iter()
                    .filter(|(i, _)| phase == "whole game" || crate::teaching::phase_of(i + 1, run.a.reviews.len()) == phase)
                    .map(|(_, p)| p.ln())
                    .collect();
                (vs.iter().sum::<f64>(), vs.len())
            })
        });
        fits.push(json!({"phase":phase,"closest_tested_profile":winner,"too_few_moves":n<8,"estimate":estimate,"scores":scores}));
    }
    // Per-move "finding rank": the weakest tested profile whose most common move is the played move.
    let mut ladder: Vec<&String> = profiles.iter().filter(|p| rank_value(p).is_some()).collect();
    ladder.sort_by(|a, b| rank_value(a).unwrap().partial_cmp(&rank_value(b).unwrap()).unwrap());
    let move_ranks: Vec<Value> = per_move
        .iter()
        .map(|(turn, entries)| {
            let finding = ladder.iter().find(|p| entries.iter().any(|(q, _, top)| q == **p && *top)).map(|p| p.to_string());
            let probs: serde_json::Map<String, Value> = entries.iter().map(|(p, prob, _)| (p.clone(), json!(round(*prob)))).collect();
            json!({"move_number": turn + 1, "move": run.a.reviews[*turn].mv, "weakest_profile_choosing_it_first": finding, "played_probability_by_profile": probs})
        })
        .collect();
    let overall = fits.first().and_then(|f| f.get("estimate").cloned()).unwrap_or(Value::Null);
    Ok(
        json!({"status":"computed","student":student,"profiles_tested":profiles,"phases":fits,"sample_turns":selected,
        "estimate":overall,"move_ranks":move_ranks,
        "caveat":"Similarity of the student's moves to human play at the tested ranks in this one game, not a rating. Small boards, handicap, opening familiarity and saturated positions affect it."}),
    )
}

/// Numeric rank scale: 20k = -20 … 1k = -1, 1d = 0, 2d = 1 … (dan ranks adjacent to 1k).
pub fn rank_value(profile: &str) -> Option<f64> {
    let r = profile.strip_prefix("rank_")?;
    let (num, suffix) = r.split_at(r.len().checked_sub(1)?);
    let n: f64 = num.parse().ok()?;
    match suffix {
        "k" => Some(-n),
        "d" => Some(n - 1.0),
        _ => None,
    }
}

pub fn rank_label(v: f64) -> String {
    let r = v.round() as i64;
    if r <= -1 {
        format!("{}k", -r)
    } else {
        format!("{}d", (r + 1).min(9))
    }
}

/// Nearest profile in the standard set for a rank value.
pub fn profile_for_value(v: f64) -> String {
    format!("rank_{}", rank_label(v.clamp(-20.0, 8.0)))
}

pub async fn analyze<F: FnMut(usize, usize, Option<&TurnEval>, &str)>(
    engine: &Engine,
    a: &GameAnalysis,
    cancel: &CancellationToken,
    done: usize,
    progress: &mut F,
) -> Result<ProbeAnalysis> {
    let start = std::time::Instant::now();
    let opts = &a.options.probes;
    let sx = a.game.size_x;
    let sy = a.game.size_y;
    let student = a.student.unwrap_or(Color::Black);
    let mut out = ProbeAnalysis::default();
    out.difficulty = a
        .turns
        .iter()
        .take(a.reviews.len())
        .filter(|t| t.to_move == student)
        .map(difficulty)
        .collect();
    for pair in a.reviews.windows(2) {
        let (opp, r) = (&pair[0], &pair[1]);
        if opp.color != student
            && r.color == student
            && opp.point_loss >= 3.0
            && r.point_loss >= 2.0
        {
            out.missed_opportunities.push(json!({"move_number":r.number,"kind":"opportunity after opponent error","opponent_move_number":opp.number,"opponent_move":opp.mv,"opponent_loss":round(opp.point_loss),"student_loss":round(r.point_loss),"points_given_back":round(opp.point_loss.min(r.point_loss)),"best":r.alternatives.first().map(|c|&c.mv),"played":r.mv,"beneficiary":student,"caveat":"Consecutive score losses establish a missed gain, not necessarily a local tactical punishment."}));
        }
    }
    if !opts.enabled {
        out.warnings.push("Teaching probes disabled".into());
        return Ok(out);
    }
    let own_rank = if student == Color::Black {
        a.game.rank_black.as_deref()
    } else {
        a.game.rank_white.as_deref()
    };
    let opp_rank = if student == Color::Black {
        a.game.rank_white.as_deref()
    } else {
        a.game.rank_black.as_deref()
    };
    let human = a
        .options
        .human_profile
        .clone()
        .or_else(|| rank_profile(own_rank))
        .or_else(|| a.options.prior_rank_value.map(profile_for_value));
    let student_source = if a.options.human_profile.is_some() { "upload" } else if rank_profile(own_rank).is_some() { "SGF rank" } else if a.options.prior_rank_value.is_some() { "working rank from earlier games" } else { "none" };
    let target = a
        .options
        .human_profile_target
        .clone()
        .or_else(|| human.as_deref().and_then(stronger).as_deref().and_then(stronger));
    let opponent = opts
        .opponent_profile
        .clone()
        .or_else(|| rank_profile(opp_rank))
        .or_else(|| human.clone());
    out.profiles = json!({"student":human,"target":target,"opponent":opponent,"student_source":student_source,"opponent_source":if opts.opponent_profile.is_some(){"explicit"}else if rank_profile(opp_rank).is_some(){"SGF rank"}else{"assumed same as student"},"small_board_caveat":sx!=19||sy!=19});
    let featured: Vec<_> = a.teaching.iter().take(opts.featured.clamp(1, 8)).collect();
    let mut run = Runner {
        engine,
        a,
        cancel,
        progress,
        done,
        total: done + featured.len() + 2,
        queries: 0,
        cache_hits: 0,
        cache: HashMap::new(),
        warnings: vec![],
        timing: BTreeMap::new(),
    };
    for tc in featured {
        if cancel.is_cancelled() {
            bail!("analysis cancelled");
        }
        let i = tc.number - 1;
        let original = &a.turns[i];
        let after = &a.turns[i + 1];
        let mut m = Moment {
            move_number: tc.number,
            mover: tc.color,
            pass_comparison: Value::Null,
            move_values: vec![],
            initiative: vec![],
            ownership_plan: Value::Null,
            local_reading: Value::Null,
            tree: vec![],
            sequences: vec![],
            human_refutations: vec![],
            rollout_comparisons: vec![],
            unavailable: BTreeMap::new(),
        };
        let mut root = match run
            .eval("candidate plans", i, &[], opts.visits, None, None)
            .await
        {
            Ok(t) => t,
            Err(e) => {
                run.unavailable(&mut m, "candidate search", e);
                original.clone()
            }
        };
        // Keep the main review's recommendation. A shallower probe must not silently replace it.
        let best = tc.best.clone().unwrap_or_else(|| tc.mv.clone());
        if let Some(k) = root.candidates.iter().position(|c| c.mv == best) {
            let preferred = root.candidates.remove(k);
            root.candidates.insert(0, preferred);
        } else if let Some(original_best) = original.candidates.iter().find(|c| c.mv == best) {
            let mut preferred = original_best.clone();
            match run
                .eval(
                    "preferred plan ownership",
                    i,
                    &[best.clone()],
                    opts.visits,
                    None,
                    None,
                )
                .await
            {
                Ok(t) => preferred.ownership = t.ownership,
                Err(e) => run.unavailable(&mut m, "preferred plan ownership", e),
            }
            root.candidates.insert(0, preferred);
        }
        if i > 0 && a.game.moves[i - 1].point.is_none() {
            m.unavailable
                .insert("pass comparison".into(), "previous move was pass".into());
        } else {
            match run
                .eval(
                    "pass comparison",
                    i,
                    &["pass".into()],
                    opts.visits,
                    None,
                    None,
                )
                .await
            {
                Ok(t) => {
                    (m.pass_comparison, m.move_values) = pass_values(original, after, &t, &tc.mv);
                    let mut line = vec!["pass".into()];
                    if let Some(c) = t.candidates.first() {
                        line.extend(c.pv.iter().take(5).cloned());
                    }
                    m.sequences.push(run.sequence(
                        "pass",
                        i,
                        line,
                        "pass search principal variation",
                        Some(&t),
                    ));
                }
                Err(e) => run.unavailable(&mut m, "pass comparison", e),
            }
        }
        if let Ok(position) = LegalPosition::from_game(&a.game, i, &a.rules) {
            m.ownership_plan = plan(&root, after, &tc.mv, sx, sy, &position.board);
        }
        if let Some(groups) = m.ownership_plan["groups"].as_array() {
            for group in groups {
                let mine = group["color"] == json!(student);
                let b = group["ownership_for_group_best"].as_f64().unwrap_or(0.0);
                let p = group["ownership_for_group_played"].as_f64().unwrap_or(0.0);
                if (mine && b - p > 0.5) || (!mine && p - b > 0.5) {
                    out.missed_opportunities.push(json!({"move_number":tc.number,"kind":if mine {"improve own group forecast"}else{"reduce opponent group forecast"},"best":best,"played":tc.mv,"group":group,"beneficiary":student,"caveat":"A plan comparison suggests a missed group-safety opportunity; this is not a proven kill or save."}));
                }
            }
        }
        if m.ownership_plan.is_null() {
            m.unavailable.insert(
                "ownership plan".into(),
                "candidate ownership not returned".into(),
            );
        }
        if let Some(c) = after.candidates.first() {
            let mut line = vec![tc.mv.clone()];
            line.extend(c.pv.clone());
            m.sequences.push(run.sequence(
                "played_engine",
                i,
                line,
                "engine principal variation",
                Some(after),
            ));
        }
        if let Some(c) = root.candidates.first() {
            m.sequences.push(run.sequence(
                "best_engine",
                i,
                c.pv.clone(),
                "engine principal variation; main-review estimate",
                Some(original),
            ));
        }
        let mut choices = vec![tc.mv.clone(), best.clone()];
        choices.extend(root.candidates.iter().take(3).map(|c| c.mv.clone()));
        let mut seen = std::collections::HashSet::new();
        choices.retain(|s| seen.insert(s.clone()));
        choices.truncate(3);
        for choice in choices {
            match run
                .eval(
                    "variation tree",
                    i,
                    &[choice.clone()],
                    opts.visits,
                    None,
                    None,
                )
                .await
            {
                Ok(child) => {
                    m.initiative
                        .push(initiative(&choice, original, &child, sx, sy));
                    let mut node = json!({"move":choice,"score_black":round(child.score_lead),"loss_for_mover":round(sign(tc.color)*(original.score_lead-child.score_lead)),"visits":child.visits,"children":[]});
                    for reply in child.candidates.iter().take(2) {
                        let mut line = vec![choice.clone(), reply.mv.clone()];
                        match run
                            .eval("variation replies", i, &line, opts.visits, None, None)
                            .await
                        {
                            Ok(grand) => {
                                let id = format!("tree_{}_{}", choice, reply.mv);
                                let mut reply_node = json!({"move":reply.mv,"score_black":round(grand.score_lead),"visits":grand.visits,"loss_for_mover":round(sign(child.to_move)*(child.score_lead-grand.score_lead)),"sequence_id":id});
                                if let Some(c) = grand.candidates.first() {
                                    line.push(c.mv.clone());
                                    match run
                                        .eval(
                                            "variation continuation",
                                            i,
                                            &line,
                                            opts.visits,
                                            None,
                                            None,
                                        )
                                        .await
                                    {
                                        Ok(leaf) => {
                                            reply_node["continuation"] = json!({"move":c.mv,"score_black":round(leaf.score_lead),"visits":leaf.visits});
                                            m.sequences.push(run.sequence(
                                                &id,
                                                i,
                                                line,
                                                "evaluated variation tree",
                                                Some(&leaf),
                                            ));
                                        }
                                        Err(e) => {
                                            line.pop();
                                            run.unavailable(&mut m, &id, e);
                                            m.sequences.push(run.sequence(
                                                &id,
                                                i,
                                                line,
                                                "evaluated variation tree",
                                                Some(&grand),
                                            ));
                                        }
                                    }
                                } else {
                                    m.sequences.push(run.sequence(
                                        &id,
                                        i,
                                        line,
                                        "evaluated variation tree",
                                        Some(&grand),
                                    ));
                                }
                                node["children"].as_array_mut().unwrap().push(reply_node);
                            }
                            Err(e) => {
                                run.unavailable(&mut m, &format!("tree {choice} {}", reply.mv), e)
                            }
                        }
                    }
                    m.tree.push(node);
                }
                Err(e) => run.unavailable(&mut m, &format!("tree {choice}"), e),
            }
        }
        match local_reading(&mut run, i, &tc.mv).await {
            Ok(v) => m.local_reading = v,
            Err(e) => run.unavailable(&mut m, "local reading", e),
        };
        if engine.config.human_model.is_some() {
            if let Some(opp) = opponent.as_deref() {
                match run
                    .eval(
                        "opponent human reply",
                        i,
                        &[tc.mv.clone()],
                        1,
                        Some(opp),
                        None,
                    )
                    .await
                {
                    Ok(t) => {
                        if let Some(policy) = t.human_policy.as_ref() {
                            if let Some((idx, p)) = policy
                                .iter()
                                .enumerate()
                                .filter(|(_, p)| **p >= 0.0)
                                .max_by(|a, b| a.1.total_cmp(b.1))
                            {
                                let mv = coordinate(idx, sx, sy);
                                let mut line = vec![tc.mv.clone(), mv.clone()];
                                match run
                                    .eval(
                                        "human reply evaluation",
                                        i,
                                        &line,
                                        opts.visits,
                                        None,
                                        None,
                                    )
                                    .await
                                {
                                    Ok(t) => {
                                        if let Some(c) = t.candidates.first() {
                                            line.extend(c.pv.iter().take(4).cloned());
                                        }
                                        let mut seq = run.sequence(
                                            "opponent_refutation",
                                            i,
                                            line,
                                            "most likely human reply; engine continuation",
                                            Some(&t),
                                        );
                                        seq.profile = Some(opp.into());
                                        m.sequences.push(seq);
                                        m.human_refutations.push(json!({"profile":opp,"opponent_color":tc.color.opponent(),"reply":mv,"probability":p,"engine_reply":after.candidates.first().map(|c|&c.mv),"score_black":round(t.score_lead),"visits":t.visits,"sequence_id":"opponent_refutation","caveat":"Most probable human move, evaluated separately; it may fail to punish."}));
                                    }
                                    Err(e) => run.unavailable(&mut m, "human reply evaluation", e),
                                }
                            }
                        } else {
                            m.unavailable
                                .insert("opponent refutation".into(), "humanPolicy missing".into());
                        }
                    }
                    Err(e) => run.unavailable(&mut m, "opponent refutation", e),
                }
            }
            for (level, profile) in [("student", human.as_deref()), ("target", target.as_deref())] {
                if let Some(profile) = profile {
                    let mut endpoints = Vec::new();
                    for (panel, mv) in [("played", tc.mv.as_str()), ("best", best.as_str())] {
                        match run
                            .rollout(
                                i,
                                mv,
                                level,
                                profile,
                                opponent.as_deref().unwrap_or(profile),
                                opts.rollout_plies.clamp(20, 30),
                            )
                            .await
                        {
                            Ok(mut seq) => {
                                seq.id = format!("{panel}_{level}");
                                endpoints.push((panel, seq.score_black, seq.moves.len()));
                                m.sequences.push(seq);
                            }
                            Err(e) => run.unavailable(&mut m, &format!("{panel}_{level}"), e),
                        }
                    }
                    if endpoints.len() == 2 {
                        m.rollout_comparisons.push(json!({"level":level,"best_minus_played_for_student":endpoints[1].1.zip(endpoints[0].1).map(|(b,p)|round(sign(student)*(b-p))),"played_plies":endpoints[0].2,"best_plies":endpoints[1].2,"caveat":"Two sampled futures; difference includes subsequent choices, not the causal value of the first move."}));
                    }
                } else {
                    m.unavailable.insert(
                        format!("human_{level}"),
                        "no profile selected or recorded in SGF".into(),
                    );
                }
            }
        } else {
            m.unavailable
                .insert("human perspectives".into(), "human model not loaded".into());
        }
        if cancel.is_cancelled() {
            bail!("analysis cancelled");
        }
        out.moments.push(m);
        run.done += 1;
        (run.progress)(run.done, run.total, None, "teaching moment complete");
    }
    // Additional endgame choices, including games without a selected teaching mistake.
    for r in a
        .reviews
        .iter()
        .filter(|r| {
            r.color == student
                && crate::teaching::phase_of(r.number, a.reviews.len()) == "endgame"
                && r.mv != "pass"
        })
        .rev()
        .take(4)
    {
        let i = r.number - 1;
        if i > 0 && a.game.moves[i - 1].point.is_none() {
            continue;
        }
        match run
            .eval(
                "endgame move values",
                i,
                &["pass".into()],
                opts.visits,
                None,
                None,
            )
            .await
        {
            Ok(pass) => {
                let (comparison, values) = pass_values(&a.turns[i], &a.turns[i + 1], &pass, &r.mv);
                out.endgame_values.push(json!({"move_number":r.number,"beneficiary":r.color,"pass_comparison":comparison,"values":values}));
            }
            Err(e) => run.warnings.push(format!("endgame move {}: {e}", r.number)),
        }
    }
    run.done += 1;
    let mut profiles: Vec<String> = ["rank_20k", "rank_15k", "rank_12k", "rank_10k", "rank_8k", "rank_6k", "rank_5k", "rank_3k", "rank_1k", "rank_1d", "rank_3d"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    for p in [human, target].into_iter().flatten() {
        if !profiles.contains(&p) {
            profiles.push(p);
        }
    }
    out.rank_fit = match rank_fit(&mut run, &profiles).await {
        Ok(v) => v,
        Err(e) => json!({"status":"unavailable","reason":e.to_string()}),
    };
    if cancel.is_cancelled() {
        bail!("analysis cancelled");
    }
    run.done += 1;
    (run.progress)(run.done, run.total, None, "teaching evidence complete");
    out.queries = run.queries;
    out.cache_hits = run.cache_hits;
    out.warnings = run.warnings;
    out.seconds_by_kind = run.timing;
    out.elapsed_seconds = start.elapsed().as_secs_f64();
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn eval(color: Color, score: f64) -> TurnEval {
        TurnEval {
            turn: 0,
            to_move: color,
            winrate: 0.5,
            score_lead: score,
            score_stdev: None,
            visits: 100,
            candidates: vec![],
            ownership: None,
            policy: None,
            human_policy: None,
            human_policy_target: None,
        }
    }
    #[test]
    fn white_values_and_policy_sampling() {
        let (p, values) = pass_values(
            &eval(Color::White, 3.0),
            &eval(Color::Black, 4.0),
            &eval(Color::Black, 5.0),
            "D4",
        );
        assert_eq!(p["best_vs_pass_points"], 2.0);
        assert_eq!(values[0]["gain_vs_pass"], 1.0);
        let policy = [-1.0, 0.0, 0.3, 0.7, 0.0];
        let mut a = 17;
        let mut b = 17;
        for _ in 0..100 {
            let x = sample(&policy, 2, 2, &mut a).unwrap();
            let y = sample(&policy, 2, 2, &mut b).unwrap();
            assert_eq!(x, y);
            assert!(x.0 == "A1" || x.0 == "B1");
        }
        assert_eq!(stronger("rank_1k").as_deref(), Some("rank_2d"));
        assert_eq!(stronger("rank_20k").as_deref(), Some("rank_18k"));
    }
}
