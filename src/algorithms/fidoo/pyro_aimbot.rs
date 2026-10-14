// Detection of flame thrower aimbot (pyro) in POV demos.
//
// Amalgam and RijiN treat the flame thrower as a projectile weapon: the target is extrapolated by the
// flame travel time and the aim angle is written into the usercmd on every tick the aimbot has a
// target, not only on attack ticks. In silent mode the angles are not applied to the local view, but
// they "show up anyways" in the networked eye angles (Amalgam comment). The networked view then
// switches between the cheater's real view and the aim angles whenever the aimbot acquires or loses
// its target, which produces sharp corners: two consecutive large view steps in opposite directions.
// A hand has to slow down to reverse, so a legit pyro reverses at a few degrees per tick, while the
// switch between the two streams happens within one tick.
//
// Every detection is a corner: two consecutive view steps whose directions differ by at least
// `min_turn` degrees, while the pyro holds a flame thrower, an enemy is within flame range in front
// of it and at most `max_enemies_in_range` enemies are around (spinning in a group is legit).
// Speeds are degrees per simulated tick, so choked usercmds (lag) don't look faster than they are.
// Signatures (see docs/pyro_aimbot.md):
// - corner:      both steps at least `min_speed` °/tick (`min_anchored_speed` if one of the three
//                samples is on the target, aimed at directly or at the flame lead point), the view
//                returns within `fast_max_return`, and the pyro dealt flame damage around it.
// - oscillation: the neighbouring sample is a corner too (aim, real, aim, ...), a sample is on the
//                target and the view returns close to where it was. No flame damage needed.
//
// Eye angles of other players are networked with ~0.35° precision and only change when the player's
// simulation time advances, so all checks run on simtime samples.

// TODO: refactor more

use std::collections::{HashMap, VecDeque};

use anyhow::Error;
use serde_json::json;
use steamid_ng::SteamID;
use tf_demo_parser::ParserState;
use tf_demo_parser::demo::gamevent::GameEvent;
use tf_demo_parser::demo::message::{GameEventMessage, Message};

use crate::base::cheat_analyser_base::{CheatAnalyserState, Player, PlayerState};
use crate::lib::algorithm::{CheatAlgorithm, Detection};
use crate::lib::parameters::AlgorithmParameters;
use crate::make_algorithm_params;
use crate::util::helpers::player_finders::valid_players;
use crate::util::helpers::{Vec3, ViewAngle};
use crate::util::nocrex::jankguard::JankGuard;

const SAMPLE_HISTORY: usize = 8;
const POSITION_HISTORY: usize = 48;
const HURT_HISTORY: usize = 64;
const TELEPORT_DIST_SQ: f32 = 256.0 * 256.0;
const FL_DUCKING: u32 = 1 << 1;
/// `TF_WEAPON_FLAMETHROWER` in player_hurt events
const WEAPON_ID_FLAMETHROWER: u16 = 25;
const TICKS_PER_SECOND: f32 = 66.67;
/// Flame particle speed (Amalgam's projectile model of the flame thrower)
const FLAME_SPEED: f32 = 1000.0;
/// Flames are spawned this far in front of the eye
const FLAME_START: f32 = 40.0;
const VELOCITY_TICKS: i64 = 6;
const BODY_BOTTOM: f32 = 5.0;
const BODY_TOP_STANDING: f32 = 80.0;
const BODY_TOP_DUCKING: f32 = 60.0;

type Tick = u32;

#[derive(Clone, Copy, Debug)]
struct Sample {
    tick: Tick,
    sim_tick: i64,
    view_angle: ViewAngle,
    eye: Vec3,
    team: u8,
    /// Holds a flame thrower and is not taunting / spawning
    flame: bool,
}

#[derive(Clone, Copy)]
struct PositionRecord {
    tick: Tick,
    position: Vec3,
    team: u8,
    ducking: bool,
}

/// Corner waiting for the flame hits around it.
struct PendingCorner {
    corner_tick: u32,
    data: serde_json::Value,
}

#[derive(Default)]
struct PlayerData {
    samples: VecDeque<Sample>,
    positions: VecDeque<PositionRecord>,
    last_sim_tick: Option<i64>,
    last_seen_tick: u32,
    last_detection: Option<u32>,
    pending: VecDeque<PendingCorner>,
}

#[derive(Default)]
pub struct PyroAimbot {
    params: Parameters,
    players: HashMap<SteamID, PlayerData>,
    /// Flame thrower damage dealt per attacker: (tick, victim)
    flame_hits: HashMap<SteamID, VecDeque<(Tick, SteamID)>>,
}

#[derive(Debug, Clone, Copy)]
pub struct Parameters {
    spawn_grace_ticks: u32,
    max_sample_gap: u32,
    max_simtime_gap: u32,
    min_step: f32,
    max_step: f32,
    min_speed: f32,
    min_anchored_speed: f32,
    max_anchor_error: f32,
    min_oscillation_speed: f32,
    oscillation_max_return: f32,
    oscillation_max_return_rel: f32,
    max_enemies_in_range: usize,
    fast_max_return: f32,
    min_turn: f32,
    max_distance: f32,
    max_target_angle: f32,
    require_hit: bool,
    hit_window_ticks: u32,
    max_lag_ticks: u32,
    min_detection_spacing: u32,
}

impl Default for Parameters {
    fn default() -> Self {
        Self {
            spawn_grace_ticks: 60,
            max_sample_gap: 3,
            max_simtime_gap: 4,
            min_step: 4.0,
            max_step: 120.0,
            min_speed: 10.0,
            min_anchored_speed: 1.0,
            max_anchor_error: 1.0,
            min_oscillation_speed: 5.0,
            oscillation_max_return: 10.0,
            oscillation_max_return_rel: 0.7,
            max_enemies_in_range: 2,
            fast_max_return: 20.0,
            min_turn: 100.0,
            max_distance: 400.0,
            max_target_angle: 35.0,
            require_hit: true,
            hit_window_ticks: 12,
            max_lag_ticks: 16,
            min_detection_spacing: 66,
        }
    }
}

fn view_step(a: &Sample, b: &Sample) -> f32 {
    // TODO: maybe replace with ViewAngle's world_delta
    a.view_angle.to_unit_vec().angle_between(b.view_angle.to_unit_vec())
}

// TODO: check if this wraps correctly / if it can be ignored
fn wrap(a: f32) -> f32 {
    (a + 180.0).rem_euclid(360.0) - 180.0
}

fn eye_position(player: &Player) -> Vec3 {
    let height = if player.flags & FL_DUCKING != 0 { 45.0 } else { 68.0 };
    Vec3 {
        x: player.position.x,
        y: player.position.y,
        z: player.position.z + height,
    }
}

/// Position of a player at `tick`, linearly interpolated between records.
fn position_at(positions: &VecDeque<PositionRecord>, tick: i64) -> Option<PositionRecord> {
    let newer_idx = positions.iter().position(|r| r.tick as i64 >= tick)?;
    let newer = positions[newer_idx];
    if newer.tick as i64 == tick {
        return Some(newer);
    }
    if newer_idx == 0 {
        return None;
    }
    let older = positions[newer_idx - 1];
    let span = newer.tick.saturating_sub(older.tick);
    if span == 0 || span > 8 {
        return None;
    }
    let f = (tick - older.tick as i64) as f32 / span as f32;
    Some(PositionRecord {
        tick: tick as u32,
        position: Vec3 {
            x: older.position.x + (newer.position.x - older.position.x) * f,
            y: older.position.y + (newer.position.y - older.position.y) * f,
            z: older.position.z + (newer.position.z - older.position.z) * f,
        },
        team: newer.team,
        ducking: newer.ducking,
    })
}

/// Angular error between the view ray and the closest point of the target's body column.
fn column_error(eye: Vec3, view_angle: ViewAngle, target: Vec3, top: f32) -> f32 {
    let dir = view_angle.to_unit_vec();
    let dx = target.x - eye.x;
    let dy = target.y - eye.y;
    let horizontal = dx.hypot(dy);
    let dir_h = dir.x.hypot(dir.y);
    if horizontal < 16.0 || dir_h < 1e-4 {
        return 180.0;
    }
    let ray_z = eye.z + dir.z / dir_h * horizontal;
    let z = ray_z.clamp(target.z + BODY_BOTTOM, target.z + top);
    dir.angle_between(Vec3 { x: dx, y: dy, z: z - eye.z })
}

/// Smallest aim error of a sample against a target seen `0..=max_lag` ticks ago, aimed at directly
/// (`lead = false`) or at the position extrapolated by the flame travel time (`lead = true`).
fn aim_error(sample: &Sample, positions: &VecDeque<PositionRecord>, max_lag: u32, lead: bool) -> f32 {
    let mut best = 180.0f32;
    for lag in (0..=max_lag as i64).step_by(2) {
        let at = sample.tick as i64 - lag;
        let Some(rec) = position_at(positions, at) else { continue };
        let top = if rec.ducking { BODY_TOP_DUCKING } else { BODY_TOP_STANDING };
        let mut target = rec.position;
        if lead {
            let Some(older) = position_at(positions, at - VELOCITY_TICKS) else { continue };
            let span = (rec.tick as i64 - older.tick as i64).max(1) as f32;
            let d = rec.position - sample.eye;
            let flight = ((d.x * d.x + d.y * d.y).sqrt() - FLAME_START).max(0.0) / FLAME_SPEED * TICKS_PER_SECOND;
            target.x += (rec.position.x - older.position.x) / span * flight;
            target.y += (rec.position.y - older.position.y) / span * flight;
        }
        best = best.min(column_error(sample.eye, sample.view_angle, target, top));
    }
    best
}

impl PyroAimbot {
    pub fn new() -> Self {
        Self::default()
    }

    fn reset(pdata: &mut PlayerData) {
        pdata.samples.clear();
        pdata.positions.clear();
        pdata.last_sim_tick = None;
    }
}

/*
struct Settings {
    spawn_grace: u32,
    max_sample_gap: u32,
    max_simtime_gap: i64,
    min_step: f32,
    max_step: f32,
    min_speed: f32,
    min_anchored_speed: f32,
    max_anchor_error: f32,
    min_oscillation_speed: f32,
    oscillation_max_return: f32,
    oscillation_max_return_rel: f32,
    max_enemies_in_range: usize,
    fast_max_return: f32,
    min_turn: f32,
    max_distance: f32,
    max_target_angle: f32,
    require_hit: bool,
    hit_window: u32,
    max_lag: u32,
    spacing: u32,
}

impl Settings {
    fn from(params: &Parameters) -> Self {
        let i = |n: &str| get_parameter_value::<i32>(params, n).max(0) as u32;
        let f = |n: &str| get_parameter_value::<f32>(params, n);
        Self {
            spawn_grace: i("spawn_grace_ticks"),
            max_sample_gap: i("max_sample_gap").max(1),
            max_simtime_gap: i("max_simtime_gap").max(1) as i64,
            min_step: f("min_step"),
            max_step: f("max_step"),
            min_speed: f("min_speed"),
            min_anchored_speed: f("min_anchored_speed"),
            max_anchor_error: f("max_anchor_error"),
            min_oscillation_speed: f("min_oscillation_speed"),
            oscillation_max_return: f("oscillation_max_return"),
            oscillation_max_return_rel: f("oscillation_max_return_rel"),
            max_enemies_in_range: i("max_enemies_in_range") as usize,
            fast_max_return: f("fast_max_return"),
            min_turn: f("min_turn"),
            max_distance: f("max_distance"),
            max_target_angle: f("max_target_angle"),
            require_hit: get_parameter_value::<bool>(params, "require_hit"),
            hit_window: i("hit_window_ticks"),
            max_lag: i("max_lag_ticks"),
            spacing: i("min_detection_spacing"),
        }
    }
}
// */

impl<'a> From<&'a mut Parameters> for AlgorithmParameters<'a> {
    fn from(value: &'a mut Parameters) -> Self {
        make_algorithm_params!(value; [
            spawn_grace_ticks,
            max_sample_gap,
            max_simtime_gap,
            min_step,
            max_step,
            min_speed,
            min_anchored_speed,
            max_anchor_error,
            min_oscillation_speed,
            oscillation_max_return,
            oscillation_max_return_rel,
            max_enemies_in_range,
            fast_max_return,
            min_turn,
            max_distance,
            max_target_angle,
            require_hit,
            hit_window_ticks,
            max_lag_ticks,
            min_detection_spacing,
        ])
    }
}

/// An enemy that can be aimed at, with its recent position records.
struct Target<'p> {
    sid: SteamID,
    positions: &'p VecDeque<PositionRecord>,
}

impl PyroAimbot {
    /// Corner shape of X -> Y -> Z: both steps (yaw scaled to arc length) and the turn between them
    /// are large. Returns (step1, step2, turn, speed), speed being the slower step in degrees per
    /// simulated tick (choked usercmds cover several ticks).
    fn corner_shape(cfg: &Parameters, x: &Sample, y: &Sample, z: &Sample) -> Option<(f32, f32, f32, f32)> {
        if !(x.flame && y.flame && z.flame) {
            return None;
        }
        for (p, q) in [(x, y), (y, z)] {
            if q.tick - p.tick > cfg.max_sample_gap || !(1..=cfg.max_simtime_gap as i64).contains(&(q.sim_tick - p.sim_tick)) {
                return None;
            }
        }
        let step = |p: &Sample, q: &Sample| {
            let dyaw = wrap(q.view_angle.yaw - p.view_angle.yaw);
            (
                dyaw * ((p.view_angle.pitch + q.view_angle.pitch) * 0.5).to_radians().cos(),
                q.view_angle.pitch - p.view_angle.pitch,
            )
        };
        let (v1, v2) = (step(x, y), step(y, z));
        let (m1, m2) = (v1.0.hypot(v1.1), v2.0.hypot(v2.1));
        if m1.min(m2) < cfg.min_step || m1.max(m2) > cfg.max_step {
            return None;
        }
        let turn = ((v1.0 * v2.0 + v1.1 * v2.1) / (m1 * m2)).clamp(-1.0, 1.0).acos().to_degrees();
        if turn < cfg.min_turn {
            return None;
        }
        let speed = (m1 / (y.sim_tick - x.sim_tick) as f32).min(m2 / (z.sim_tick - y.sim_tick) as f32);
        Some((m1, m2, turn, speed))
    }

    /// Sharp corner A -> B -> C over the samples before the newest one (D) while engaging an enemy.
    fn check_corner(cfg: &Parameters, samples: &VecDeque<Sample>, targets: &[Target]) -> Option<serde_json::Value> {
        let n = samples.len();
        if n < 4 {
            return None;
        }
        let (a, b, c, d) = (&samples[n - 4], &samples[n - 3], &samples[n - 2], &samples[n - 1]);
        let before = (n >= 5).then(|| &samples[n - 5]);
        let (m1, m2, turn, speed) = Self::corner_shape(cfg, a, b, c)?;
        if speed < cfg.min_speed.min(cfg.min_anchored_speed).min(cfg.min_oscillation_speed) {
            return None;
        }
        // Engaged: the closest enemy within flame range in front of the pyro
        let mut engaged: Option<(&Target, f32, f32)> = None;
        let mut in_range = 0;
        for target in targets {
            let Some(rec) = position_at(target.positions, b.tick as i64) else { continue };
            let dx = rec.position.x - b.eye.x;
            let dy = rec.position.y - b.eye.y;
            let distance = dx.hypot(dy);
            if distance > cfg.max_distance {
                continue;
            }
            in_range += 1;
            let off = wrap(b.view_angle.yaw - dy.atan2(dx).to_degrees()).abs();
            if off > cfg.max_target_angle {
                continue;
            }
            if engaged.is_none_or(|e| distance < e.1) {
                engaged = Some((target, distance, off));
            }
        }
        // Spinning around in a group of enemies produces fast corners too
        if in_range > cfg.max_enemies_in_range {
            return None;
        }
        let (target, distance, off) = engaged?;
        let errors = |s: &Sample| {
            (
                aim_error(s, target.positions, cfg.max_lag_ticks, false),
                aim_error(s, target.positions, cfg.max_lag_ticks, true),
            )
        };
        let (err_a, lead_a) = errors(a);
        let (err_b, lead_b) = errors(b);
        let (err_c, lead_c) = errors(c);
        // One of the samples is on the target (aimed at directly or at the flame lead point)
        let anchor = [err_a, lead_a, err_b, lead_b, err_c, lead_c].into_iter().fold(180.0f32, f32::min);
        let anchored = anchor <= cfg.max_anchor_error;
        let returned = view_step(a, c);
        // Oscillation: the neighbouring sample is a corner too (aim, real, aim, ...), and the view
        // returns close to where it was
        let train = before.is_some_and(|x| Self::corner_shape(cfg, x, a, b).is_some()) || Self::corner_shape(cfg, b, c, d).is_some();
        let oscillation = train
            && anchored
            && speed >= cfg.min_oscillation_speed
            && (returned <= cfg.oscillation_max_return || returned <= cfg.oscillation_max_return_rel * m1.max(m2));
        // Fast corner: the view comes back close to where it was. A legit swing through a fight
        // reverses only part of the way.
        let fast = (speed >= cfg.min_speed || (anchored && speed >= cfg.min_anchored_speed)) && returned <= cfg.fast_max_return;
        if !oscillation && !fast {
            return None;
        }
        Some(json!({
            "type": if oscillation { "oscillation" } else { "corner" },
            "tick": b.tick,
            "target": target.sid,
            "step1_deg": m1,
            "step2_deg": m2,
            "turn_deg": turn,
            "speed_deg_per_tick": speed,
            "anchor_error_deg": anchor,
            "return_deg": returned,
            "train": train,
            "distance": distance,
            "target_angle_deg": off,
            "enemies_in_range": in_range,
            "aim_error_deg": [err_a, err_b, err_c],
            "lead_error_deg": [lead_a, lead_b, lead_c],
            "gap_ticks": [b.tick - a.tick, c.tick - b.tick],
            "sim_gap_ticks": [b.sim_tick - a.sim_tick, c.sim_tick - b.sim_tick],
        }))
    }

    /// Ticks from the corner to the closest flame hit of `attacker` within the hit window.
    fn hit_offset(&self, attacker: SteamID, corner_tick: u32, window: u32) -> Option<i64> {
        self.flame_hits
            .get(&attacker)?
            .iter()
            .map(|&(t, _)| t as i64 - corner_tick as i64)
            .filter(|dt| dt.unsigned_abs() <= window as u64)
            .min_by_key(|dt| dt.abs())
    }
}

impl CheatAlgorithm for PyroAimbot {
    fn enabled_by_default(&self) -> bool {
        true
    }

    fn algorithm_name(&self) -> &'static str {
        "fidoo/pyro_aimbot"
    }

    fn params(&mut self) -> Option<AlgorithmParameters<'_>> {
        Some(AlgorithmParameters::from(&mut self.params))
    }

    fn on_message(
        &mut self,
        message: &Message,
        state: &CheatAnalyserState,
        _parser_state: &ParserState,
        _jg: &JankGuard,
        tick: tf_demo_parser::demo::data::DemoTick,
    ) -> Result<Vec<Detection>, Error> {
        if let Message::GameEvent(GameEventMessage {
            event: GameEvent::PlayerHurt(hurt),
            ..
        }) = message
            && hurt.weapon_id == WEAPON_ID_FLAMETHROWER
            && hurt.attacker != hurt.user_id
            && let (Some(attacker), Some(victim)) = (
                state.get_steamid_from_userid(hurt.attacker.into()),
                state.get_steamid_from_userid(hurt.user_id.into()),
            )
        {
            let hits = self.flame_hits.entry(attacker).or_default();
            hits.push_back((u32::from(tick), victim));
            if hits.len() > HURT_HISTORY {
                hits.pop_front();
            }
        }
        Ok(vec![])
    }

    fn on_tick(&mut self, state: &CheatAnalyserState, _: &ParserState, jg: &JankGuard) -> Result<Vec<Detection>, Error> {
        let cfg = &self.params;
        let tick = u32::from(state.tick);

        // 1. Record positions and new eye angle samples
        let mut updated: Vec<SteamID> = Vec::new();

        for (player, _info, sid) in valid_players(state.players.iter()) {
            let pdata = self.players.entry(sid).or_default();
            if !player.in_pvs || player.state != PlayerState::Alive {
                Self::reset(pdata);
                continue;
            }
            if pdata.last_seen_tick > 0 && tick.saturating_sub(pdata.last_seen_tick) > 16 {
                Self::reset(pdata);
            }
            if let Some(last) = pdata.positions.back() {
                let d = player.position - last.position;
                if d.x * d.x + d.y * d.y + d.z * d.z > TELEPORT_DIST_SQ {
                    Self::reset(pdata);
                }
            }
            pdata.last_seen_tick = tick;
            let sim_tick = state.sim_tick(player);
            if pdata.last_sim_tick == Some(sim_tick) {
                continue;
            }
            pdata.last_sim_tick = Some(sim_tick);
            if pdata.positions.back().is_none_or(|r| r.tick < tick) {
                pdata.positions.push_back(PositionRecord {
                    tick,
                    position: player.position,
                    team: player.team as u8,
                    ducking: player.flags & FL_DUCKING != 0,
                });
                if pdata.positions.len() > POSITION_HISTORY {
                    pdata.positions.pop_front();
                }
            }
            if pdata.samples.back().is_some_and(|s| s.tick == tick) {
                continue;
            }
            let flame_thrower = player
                .active_weapon
                .and_then(|w| state.weapons.get(&w))
                .is_some_and(|w| w.class_name == "CTFFlameThrower");
            let since_spawn = jg.spawned(&sid, tick).min(jg.teleported(&sid, tick));
            pdata.samples.push_back(Sample {
                tick,
                sim_tick,
                view_angle: player.view_angle,
                eye: eye_position(player),
                team: player.team as u8,
                flame: flame_thrower && since_spawn >= cfg.spawn_grace_ticks && !player.is_taunting(),
            });
            if pdata.samples.len() > SAMPLE_HISTORY {
                pdata.samples.pop_front();
            }
            if flame_thrower {
                updated.push(sid);
            }
        }

        // 2. Look for corners of pyros with a new sample
        for sid in updated {
            let Some(mut pdata) = self.players.remove(&sid) else { continue };
            let team = pdata.samples.back().map(|s| s.team).unwrap_or(0);
            let targets: Vec<Target> = self
                .players
                .iter()
                .filter_map(|(s, d)| {
                    let r = d.positions.back()?;
                    (r.team != team && tick.saturating_sub(r.tick) <= 8).then_some(Target {
                        sid: *s,
                        positions: &d.positions,
                    })
                })
                .collect();
            if let Some(data) = Self::check_corner(&cfg, &pdata.samples, &targets) {
                let corner_tick = data["tick"].as_u64().unwrap_or(tick as u64) as u32;
                pdata.pending.push_back(PendingCorner { corner_tick, data });
            }
            drop(targets);
            self.players.insert(sid, pdata);
        }

        // 3. Finalize corners once the flame hits after them have arrived
        let mut out = Vec::new();
        let sids: Vec<SteamID> = self.players.iter().filter(|(_, d)| !d.pending.is_empty()).map(|(s, _)| *s).collect();
        for sid in sids {
            loop {
                let pdata = self.players.get_mut(&sid).unwrap();
                let Some(front) = pdata.pending.front() else { break };
                if tick < front.corner_tick + cfg.hit_window_ticks {
                    break;
                }
                let PendingCorner { corner_tick, mut data } = pdata.pending.pop_front().unwrap();
                let hit = self.hit_offset(sid, corner_tick, cfg.hit_window_ticks);
                // Oscillations show the aim stream itself and need no hit
                if cfg.require_hit && hit.is_none() && data["type"] == "corner" {
                    continue;
                }
                let pdata = self.players.get_mut(&sid).unwrap();
                if pdata
                    .last_detection
                    .is_some_and(|last| corner_tick.saturating_sub(last) < cfg.min_detection_spacing)
                {
                    continue;
                }
                pdata.last_detection = Some(corner_tick);
                let (class, weapon) = state.get_player_class_and_weapon_by_sid(sid);
                data["hit_offset_ticks"] = json!(hit);
                data["class"] = json!(class);
                data["weapon"] = json!(weapon);
                out.push(Detection {
                    tick: corner_tick,
                    algorithm: self.algorithm_name(),
                    player: sid,
                    data,
                });
            }
        }
        Ok(out)
    }
}
