// TODO: refactor more
use std::collections::{HashMap, HashSet, VecDeque};

use anyhow::Error;
use serde_json::json;
use steamid_ng::SteamID;
use tf_demo_parser::demo::message::Message;
use tf_demo_parser::demo::message::packetentities::{EntityId, UpdateType};
use tf_demo_parser::demo::parser::analyser::Class;
use tf_demo_parser::demo::sendprop::SendPropIdentifier;
use tf_demo_parser::{MessageType, ParserState};

use crate::base::cheat_analyser_base::{CheatAnalyserState, Player, PlayerState};
use crate::lib::algorithm::{CheatAlgorithm, Detection};
use crate::lib::parameters::AlgorithmParameters;
use crate::make_algorithm_params;
use crate::util::helpers::geometry::ViewAngleDelta;
use crate::util::helpers::{KeyValueVec, Vec3, ViewAngle, handle_to_entid};
use crate::util::nocrex::jankguard::JankGuard;

const HISTORY_TICKS: u32 = 40;
const TELEPORT_THRESHOLD: f32 = 400.0;

// Standard and transition eye heights: ducking (45), intermediate crouch transitions, and standing (68, 72)
const EYE_HEIGHTS: [f32; 6] = [45.0, 50.0, 56.0, 62.0, 68.0, 72.0];

// Candidate target convergence distances in Hammer Units
const CANDIDATE_DISTANCES: [f32; 9] = [24.0, 48.0, 96.0, 192.0, 384.0, 768.0, 1536.0, 2000.0, 4000.0];

type Tick = u32;

#[derive(Debug, Clone)]
struct PlayerSnapshot {
    tick: Tick,
    position: Vec3,
    angle: ViewAngle,
    simtime: u16,
    in_pvs: bool,
    state: PlayerState,
    is_taunting: bool,
}

impl PlayerSnapshot {
    fn new(tick: Tick, player: &Player) -> Self {
        Self {
            tick,
            position: player.position,
            angle: player.view_angle,
            simtime: player.simtime,
            in_pvs: player.in_pvs,
            state: player.state,
            is_taunting: player.is_taunting() || (player.cond & (1 << 7)) != 0,
        }
    }
}

#[derive(Debug, Clone)]
// TODO: check if `String`s are necessary
struct LaunchCandidate {
    tick: Tick,
    player: SteamID,
    player_class: Class,
    weapon_name: String,
    player_ping: u16,
    projectile: String,
    projectile_entity: u32,
    serial: u32,
    origin: Vec3,
    initial_velocity: Vec3,
    launch_direction: Vec3,
    owner_distance: f32,
    pipe_type: Option<i64>,
}

#[derive(Debug, Clone)]
struct TentativeDetection {
    player: SteamID,
    tick: Tick,

    class: Class,
    weapon_name: String,
    projectile_name: String,
    projectile_entity: u32,
    projectile_serial: u32,
    origin: Vec3,
    initial_velocity: Vec3,
    launch_angle: ViewAngle,
    view_angle: ViewAngle,
    view_tick: Tick,
    raw_angle_delta: f32,
    compensated_angle_delta: f32,
    view_angle_delta: ViewAngleDelta,
    owner_distance: f32,
    pipe_type: Option<i64>,
    sample_count: usize,
}

#[derive(Debug, Clone, Default)]
pub struct Psilent5 {
    histories: HashMap<SteamID, VecDeque<PlayerSnapshot>>,
    pending: Vec<LaunchCandidate>,
    tentative: Vec<TentativeDetection>,
    /// HashMap<(EntityIndex, SerialNumber), FirstTick>
    observations: HashMap<(u32, u32), Tick>,
    params: Parameters,

    /// HashMap<Player, (EntityId, SerialNumber)>
    suspicious_launches: HashMap<SteamID, HashSet<(u32, u32)>>,
    last_detection: KeyValueVec<SteamID, Tick>,
}

#[derive(Debug, Clone, Copy)]
pub struct Parameters {
    pub angle_threshold_suspicious: f32,
    pub angle_threshold_blatant: f32,
    pub lookaround_ticks: u32,
    pub max_owner_distance: f32,
    pub minimum_samples: usize,
    // TODO: implement minimum launches to on-tick detections
    pub minimum_suspicious_launches: usize,
    /// amount of ticks that has to pass between 2 detections
    pub detection_cooldown: u32,
}

impl Default for Parameters {
    fn default() -> Self {
        Self {
            angle_threshold_suspicious: 6.0,
            angle_threshold_blatant: 25.0,
            lookaround_ticks: 18,
            max_owner_distance: 115.0,
            minimum_samples: 2,
            minimum_suspicious_launches: 2,
            detection_cooldown: 3,
        }
    }
}

impl Psilent5 {
    pub fn new() -> Self {
        Self::default()
    }

    fn record_state(&mut self, state: &CheatAnalyserState) {
        let tick = u32::from(state.tick);
        for player in &state.players {
            let Some(steam_id) = player_id(player) else {
                continue;
            };
            self.record_snapshot(steam_id, PlayerSnapshot::new(tick, player));
        }
        self.histories.retain(|_, history| {
            while history.front().is_some_and(|snapshot| tick.saturating_sub(snapshot.tick) > HISTORY_TICKS) {
                history.pop_front();
            }
            !history.is_empty()
        });
    }

    fn record_snapshot(&mut self, steam_id: SteamID, snapshot: PlayerSnapshot) {
        let history = self.histories.entry(steam_id).or_default();
        if let Some(previous) = history.back() && previous.tick == snapshot.tick
                && previous.simtime == snapshot.simtime
                && (previous.angle.yaw - snapshot.angle.yaw).abs() < 0.001
                && (previous.angle.pitch - snapshot.angle.pitch).abs() < 0.001
                // TODO: consider using length_square()
                && (previous.position - snapshot.position).length() < 0.01
        {
            return;
        }
        history.push_back(snapshot);
    }

    fn resolve_pending(&mut self, current_tick: Tick, flush: bool) -> Vec<TentativeDetection> {
        let lookaround_ticks = self.params.lookaround_ticks;
        let mut waiting = Vec::new();
        let mut detections = Vec::new();

        for candidate in std::mem::take(&mut self.pending).into_iter() {
            if !self.histories.contains_key(&candidate.player) {
                if !flush {
                    waiting.push(candidate);
                }
                continue;
            }

            let ping_ticks = ((candidate.player_ping as f32 / 15.0).round() as u32).min(20);
            let effective_lookaround = (lookaround_ticks + ping_ticks).clamp(18, 32);

            if !flush && current_tick < candidate.tick.saturating_add(effective_lookaround) {
                waiting.push(candidate);
                continue;
            }

            if let Some(detection) = evaluate_candidate(&self.params, &self.histories, candidate) {
                detections.push(detection);
            }
        }
        self.pending = waiting;
        detections
    }

    const ALGORITHM_NAME: &'static str = "fidoo/psilent5";
}

impl CheatAlgorithm for Psilent5 {
    fn enabled_by_default(&self) -> bool {
        true
    }

    fn algorithm_name(&self) -> &'static str {
        Self::ALGORITHM_NAME
    }

    fn params(&mut self) -> Option<AlgorithmParameters<'_>> {
        let params = make_algorithm_params!(self.params; [
            angle_threshold_suspicious,
            angle_threshold_blatant,
            lookaround_ticks,
            max_owner_distance,
            minimum_samples,
            minimum_suspicious_launches,
        ]);
        Some(params)
    }

    fn handled_messages(&self) -> Result<Vec<MessageType>, bool> {
        Ok(vec![MessageType::PacketEntities])
    }

    fn on_tick(&mut self, state: &CheatAnalyserState, _: &ParserState, _jg: &JankGuard) -> Result<Vec<Detection>, Error> {
        self.record_state(state);
        let resolved = self.resolve_pending(state.tick.into(), false);

        for td in resolved.into_iter() {
            if self
                .suspicious_launches
                .entry(td.player)
                .or_default()
                .insert((td.projectile_entity, td.projectile_serial))
            {
                self.tentative.push(td);
            };
        }

        let mut detections = Vec::new();

        for td in std::mem::take(&mut self.tentative).into_iter() {
            let delta_is_blatant = td.compensated_angle_delta >= self.params.angle_threshold_blatant;
            let suspicious_launches_exceeded = match self
                .suspicious_launches
                .get(&td.player)
                .map(|c| c.len() >= self.params.minimum_suspicious_launches)
            {
                Some(v) => v,
                None => {
                    tracing::error!(
                        "no recorded suspicious_launches for {}/{}, yet detections contain one",
                        u64::from(td.player),
                        td.player.steam3()
                    );
                    false
                }
            };
            let tick_cooldown_is_ready = self
                .last_detection
                .get(&td.player)
                .map(|last_detection| td.tick.saturating_sub(*last_detection) > self.params.detection_cooldown)
                .unwrap_or(true);

            if (delta_is_blatant || suspicious_launches_exceeded) && tick_cooldown_is_ready {
                _ = self.last_detection.insert(td.player, td.tick);
                detections.push(Detection::from(td));
            } else {
                self.tentative.push(td);
            }
        }

        Ok(detections)
    }

    fn on_message(
        &mut self,
        message: &Message,
        state: &CheatAnalyserState,
        parser_state: &ParserState,
        _jg: &JankGuard,
        _: tf_demo_parser::demo::data::DemoTick,
    ) -> Result<Vec<Detection>, Error> {
        let Message::PacketEntities(message) = message else {
            return Ok(vec![]);
        };
        self.record_state(state);
        let tick = u32::from(state.tick);

        for entity in &message.entities {
            if entity.update_type != UpdateType::Enter {
                continue;
            }
            let Some(class) = parser_state
                .server_classes
                .get(usize::from(entity.server_class))
                .map(|class| class.name.as_str())
                .filter(|class| is_accepted_projectile(class))
            else {
                continue;
            };

            let props: Vec<_> = entity.props(parser_state).collect();

            // 100% Reliable Networked Owner Verification
            let Some(owner_steam_id) = resolve_owner_steam_id(&props, state) else {
                continue;
            };

            let Some(player) = state.players.iter().find(|p| player_id(p) == Some(owner_steam_id)) else {
                continue;
            };

            let is_taunting = player.is_taunting() || (player.cond & (1 << 7)) != 0;
            if !player.in_pvs || player.state != PlayerState::Alive || is_taunting {
                continue;
            }

            // Extract projectile spawn origin
            let Some(origin) = projectile_vector(
                &props,
                &[
                    ("DT_TFBaseRocket", "m_vecOrigin"),
                    ("DT_TFWeaponBaseGrenadeProj", "m_vecOrigin"),
                    ("DT_BaseEntity", "m_vecOrigin"),
                ],
            ) else {
                continue;
            };

            // Extract projectile launch velocity / direction
            let Some(initial_velocity) = extract_initial_velocity(class, &props) else {
                continue;
            };

            // Filter out deflected or bounced projectiles
            if is_deflected_or_touched(&props) {
                continue;
            }

            let Some(launch_direction) = normalized(initial_velocity) else {
                continue;
            };

            let weapon_name = state.get_player_weapon(player);
            if is_excluded_weapon(weapon_name) {
                continue;
            }
            let pipe_type = projectile_integer(&props, &[("DT_TFProjectile_Pipebomb", "m_iType")]);

            let max_owner_distance = self.params.max_owner_distance;
            let offset = origin - player.position;
            let owner_distance = offset.length();
            if !owner_distance.is_finite() || owner_distance > max_owner_distance {
                continue;
            }

            let observation_key = (u32::from(entity.entity_index), entity.serial_number);
            match self.observations.get(&observation_key) {
                None => {
                    self.observations.insert(observation_key, tick);
                }
                Some(&first_tick) if tick.saturating_sub(first_tick) <= 2 => {
                    // Refine candidate within same initial spawn burst across adjacent ticks
                }
                Some(_) => {
                    // Re-entering PVS long after initial spawn (e.g. arrow stuck in wall), ignore
                    continue;
                }
            }

            self.record_snapshot(owner_steam_id, PlayerSnapshot::new(tick, player));
            self.pending.push(LaunchCandidate {
                tick,
                player: owner_steam_id,
                player_class: player.class,
                weapon_name: weapon_name.to_owned(),
                player_ping: player.ping,
                projectile: class.to_string(),
                projectile_entity: entity.entity_index.into(),
                serial: entity.serial_number,
                origin,
                initial_velocity,
                launch_direction,
                owner_distance,
                pipe_type,
            });
        }

        Ok(vec![])
    }

    fn finish(&mut self) -> Result<Vec<Detection>, Error> {
        let resolved = self.resolve_pending(u32::MAX, true);
        self.tentative.extend(resolved);

        let unique = get_unique_tentative_detections(std::mem::take(&mut self.tentative));

        let minimum_launches = self.params.minimum_suspicious_launches;
        let mut launch_counts = HashMap::<SteamID, usize>::new();
        for (player, _, _) in unique.keys() {
            *launch_counts.entry(*player).or_default() += 1;
        }

        // Emit if minimum suspicious launches met OR if blatant high-delta single shot
        let mut detections: Vec<_> = unique
            .into_values()
            .into_iter()
            .filter(|tentative| {
                let delta = tentative.compensated_angle_delta;
                delta >= self.params.angle_threshold_blatant || launch_counts.get(&tentative.player).is_some_and(|count| *count >= minimum_launches)
            })
            .map(Detection::from)
            .collect();
        // TODO: check if sorting is necessary
        detections.sort_by_key(|detection| (detection.tick, u64::from(detection.player)));
        Ok(detections)
    }
}

fn get_unique_tentative_detections(tentative_detections: Vec<TentativeDetection>) -> KeyValueVec<(SteamID, u32, u32), TentativeDetection> {
    let mut unique = KeyValueVec::<(SteamID, u32, u32), TentativeDetection>::new();
    for tentative in tentative_detections.into_iter() {
        let key = (tentative.player, tentative.projectile_entity, tentative.projectile_serial);
        let new_delta = tentative.compensated_angle_delta;
        match unique.get(&key) {
            Some(existing) if existing.compensated_angle_delta >= new_delta => {}
            _ => {
                _ = unique.insert(key, tentative);
            }
        }
    }
    unique
}

fn evaluate_candidate(params: &Parameters, histories: &HashMap<SteamID, VecDeque<PlayerSnapshot>>, candidate: LaunchCandidate) -> Option<TentativeDetection> {
    // Exclude random spread weapons (Beggar's Bazooka) and zero-damage practice weapons (Rocket Jumper, Sticky Jumper)
    if is_excluded_weapon(&candidate.weapon_name) {
        return None;
    }

    let angle_threshold = params.angle_threshold_suspicious;
    let lookaround_ticks = params.lookaround_ticks;
    let minimum_samples = params.minimum_samples;
    let history = histories.get(&candidate.player)?;
    let ping_ticks = ((candidate.player_ping as f32 / 15.0).round() as u32).min(20);
    let effective_lookaround = (lookaround_ticks + ping_ticks).clamp(18, 32);
    let start_tick = candidate.tick.saturating_sub(effective_lookaround);
    let end_tick = candidate.tick.saturating_add(effective_lookaround);

    let samples: Vec<_> = history
        .iter()
        .filter(|sample| (start_tick..=end_tick).contains(&sample.tick) && sample.in_pvs && sample.state == PlayerState::Alive && !sample.is_taunting)
        .collect();

    if samples.len() < minimum_samples {
        return None;
    }

    if history.iter().any(|s| s.tick.abs_diff(candidate.tick) <= 2 && s.is_taunting) {
        return None;
    }

    for window in samples.windows(2) {
        let (s1, s2) = (window[0], window[1]);
        let pos_delta = s2.position - s1.position;
        // TODO: consider rewriting to use `length_square()`
        let distance = pos_delta.length();
        if distance > TELEPORT_THRESHOLD {
            return None;
        }
    }

    let launch_angles = ViewAngle::from(candidate.launch_direction);
    let speed = candidate.initial_velocity.length();
    let is_pipebomb = candidate.projectile == "CTFGrenadePipebombProjectile";

    let mut raw_delta = f32::INFINITY;
    let mut compensated_delta = f32::INFINITY;
    let mut best_view = ViewAngle::default();
    let mut best_tick = candidate.tick;

    // Generate test views from recorded snapshots and interpolated sub-tick frames
    let mut test_views = Vec::new();
    for sample in &samples {
        test_views.push((sample.tick, sample.position, sample.angle));
    }

    for window in samples.windows(2) {
        let (s1, s2) = (window[0], window[1]);
        let tick_gap = s2.tick.saturating_sub(s1.tick);
        if tick_gap > 16 {
            continue;
        }

        let steps = if tick_gap == 0 { 4 } else { (tick_gap.min(8) * 2).max(4) };
        for step in 1..steps {
            let fraction = step as f32 / steps as f32;
            let va_diff = s1.angle.viewangle_delta(s2.angle);
            // TODO: check if pitch clamping is really necessary.
            // if so, manually calculate `interp`. check commit 556b7c379c8e for how this was done
            let interp_angle = s1.angle + va_diff * fraction;
            let interp_pos = s1.position + (s2.position - s1.position) * fraction;
            let interp_tick_float = s1.tick as f32 + (s2.tick as f32 - s1.tick as f32) * fraction;
            let interp_tick = interp_tick_float.round() as u32;
            test_views.push((interp_tick, interp_pos, interp_angle));
        }
    }

    // Continuous Yaw Compensation & Convergence Evaluation
    for (tick, pos, view) in test_views {
        let current_raw = view.world_angle_delta(launch_angles);
        if current_raw < raw_delta {
            raw_delta = current_raw;
        }

        let v_view = view.to_unit_vec();
        let v_up = angles_to_up_vector(view);

        // For CTFGrenadePipebombProjectile, Source engine adds vecUp * 200.0f
        // (CTFGrenadePipebombProjectile::Create). We check both with upward arc and unboosted (e.g. Loose Cannon)
        let arc_boost_candidates: &[(bool, f32)] = if is_pipebomb && speed > 210.0 {
            &[(true, 200.0), (false, 0.0)]
        } else {
            &[(false, 0.0)]
        };

        for &eye_height in &EYE_HEIGHTS {
            let eye = Vec3 { z: pos.z + eye_height, ..pos };

            for &(use_arc, arc_boost) in arc_boost_candidates {
                // 1. Direct view angle alignment (infinite distance target: d -> infinity)
                let expected_direct = if use_arc {
                    let fwd_speed = (speed * speed - arc_boost * arc_boost).max(0.0).sqrt();
                    let v = Vec3 {
                        x: v_view.x * fwd_speed + v_up.x * arc_boost,
                        y: v_view.y * fwd_speed + v_up.y * arc_boost,
                        z: v_view.z * fwd_speed + v_up.z * arc_boost,
                    };
                    normalized(v).unwrap_or(v_view)
                } else {
                    v_view
                };
                let direct_angles = ViewAngle::from(expected_direct);
                let direct_delta = direct_angles.world_angle_delta(launch_angles);
                if direct_delta < compensated_delta {
                    compensated_delta = direct_delta;
                    best_view = view;
                    best_tick = tick;
                }

                // 2. Analytical closest approach distance d* along sightline ray
                let w = eye - candidate.origin;
                let c = candidate.launch_direction.dot(expected_direct);
                let denom = 1.0 - c * c;
                if denom > 1e-6 {
                    let d_star = (c * candidate.launch_direction.dot(w) - expected_direct.dot(w)) / denom;
                    if d_star > 10.0 && d_star < 8000.0 {
                        let target = Vec3 {
                            x: eye.x + v_view.x * d_star,
                            y: eye.y + v_view.y * d_star,
                            z: eye.z + v_view.z * d_star,
                        };
                        if let Some(mut expected_dir) = normalized(target - candidate.origin) {
                            if use_arc {
                                let fwd_speed = (speed * speed - arc_boost * arc_boost).max(0.0).sqrt();
                                let v = Vec3 {
                                    x: expected_dir.x * fwd_speed + v_up.x * arc_boost,
                                    y: expected_dir.y * fwd_speed + v_up.y * arc_boost,
                                    z: expected_dir.z * fwd_speed + v_up.z * arc_boost,
                                };
                                if let Some(n) = normalized(v) {
                                    expected_dir = n;
                                }
                            }
                            let expected_angles = ViewAngle::from(expected_dir);
                            let delta = launch_angles.world_angle_delta(expected_angles);
                            if delta < compensated_delta {
                                compensated_delta = delta;
                                best_view = view;
                                best_tick = tick;
                            }
                        }
                    }
                }

                // 3. Ground plane impact distance for downward shots / rocket jumps
                if view.pitch > 20.0 && v_view.z < -0.05 {
                    let d_ground = eye_height / (-v_view.z);
                    if d_ground > 10.0 && d_ground < 1500.0 {
                        let target = Vec3 {
                            x: eye.x + v_view.x * d_ground,
                            y: eye.y + v_view.y * d_ground,
                            z: pos.z,
                        };
                        if let Some(mut expected_dir) = normalized(target - candidate.origin) {
                            if use_arc {
                                let fwd_speed = (speed * speed - arc_boost * arc_boost).max(0.0).sqrt();
                                let v = Vec3 {
                                    x: expected_dir.x * fwd_speed + v_up.x * arc_boost,
                                    y: expected_dir.y * fwd_speed + v_up.y * arc_boost,
                                    z: expected_dir.z * fwd_speed + v_up.z * arc_boost,
                                };
                                if let Some(n) = normalized(v) {
                                    expected_dir = n;
                                }
                            }
                            let expected_angles = ViewAngle::from(expected_dir);
                            let delta = launch_angles.world_angle_delta(expected_angles);
                            if delta < compensated_delta {
                                compensated_delta = delta;
                                best_view = view;
                                best_tick = tick;
                            }
                        }
                    }
                }

                // 4. Candidate engagement distances covering close to far ranges
                for &dist in &CANDIDATE_DISTANCES {
                    let target = Vec3 {
                        x: eye.x + v_view.x * dist,
                        y: eye.y + v_view.y * dist,
                        z: eye.z + v_view.z * dist,
                    };
                    if let Some(mut expected_dir) = normalized(target - candidate.origin) {
                        if use_arc {
                            let fwd_speed = (speed * speed - arc_boost * arc_boost).max(0.0).sqrt();
                            let v = Vec3 {
                                x: expected_dir.x * fwd_speed + v_up.x * arc_boost,
                                y: expected_dir.y * fwd_speed + v_up.y * arc_boost,
                                z: expected_dir.z * fwd_speed + v_up.z * arc_boost,
                            };
                            if let Some(n) = normalized(v) {
                                expected_dir = n;
                            }
                        }
                        let expected_angles = ViewAngle::from(expected_dir);
                        let delta = launch_angles.world_angle_delta(expected_angles);
                        if delta < compensated_delta {
                            compensated_delta = delta;
                            best_view = view;
                            best_tick = tick;
                        }
                    }
                }
            }
        }
    }

    let is_downward_rocket = candidate.launch_direction.z < -0.35 && candidate.projectile == "CTFProjectile_Rocket";
    let effective_threshold = if is_downward_rocket { angle_threshold + 1.5 } else { angle_threshold };

    if !compensated_delta.is_finite() || compensated_delta < effective_threshold {
        return None;
    }

    let yaw_diff = (launch_angles.yaw - best_view.yaw).rem_euclid(360.0);
    let yaw_delta = if yaw_diff > 180.0 { 360.0 - yaw_diff } else { yaw_diff };
    let pitch_delta = (launch_angles.pitch - best_view.pitch).abs();
    let view_angle_delta = ViewAngleDelta {
        yaw: yaw_delta,
        pitch: pitch_delta,
    };

    Some(TentativeDetection {
        player: candidate.player,
        tick: candidate.tick,
        class: candidate.player_class,
        weapon_name: candidate.weapon_name,
        projectile_name: candidate.projectile,
        projectile_entity: candidate.projectile_entity,
        projectile_serial: candidate.serial,
        origin: candidate.origin,
        initial_velocity: candidate.initial_velocity,
        launch_angle: launch_angles,
        view_angle: best_view,
        view_tick: best_tick,
        raw_angle_delta: raw_delta,
        compensated_angle_delta: compensated_delta,
        view_angle_delta,
        owner_distance: candidate.owner_distance,
        pipe_type: candidate.pipe_type,
        sample_count: samples.len(),
    })
}

// fn detection_delta(detection: &TentativeDetection) -> f32 {
//     detection.compensated_angle_delta
// }

fn is_excluded_weapon(weapon_name: &str) -> bool {
    weapon_name.contains("Beggar") || weapon_name.contains("Rocket Jumper") || weapon_name.contains("Sticky Jumper")
}

// 100% Reliable Owner Resolution:
// Resolves entity handles from m_hThrower (grenade/sticky priority) and m_hOwnerEntity (rockets/flares/arrows/etc.)
// Maps to player entity and SteamID64
fn resolve_owner_steam_id(props: &[tf_demo_parser::demo::sendprop::SendProp], state: &CheatAnalyserState) -> Option<SteamID> {
    const OWNER: SendPropIdentifier = SendPropIdentifier::new("DT_BaseEntity", "m_hOwnerEntity");
    const THROWER: SendPropIdentifier = SendPropIdentifier::new("DT_BaseGrenade", "m_hThrower");

    // Priority 1: m_hThrower (pipes, stickies)
    if let Some(prop) = props.iter().find(|p| p.identifier == THROWER)
        && let Ok(handle) = i64::try_from(&prop.value)
        && let Some(steam_id) = steam_id_from_entid(handle_to_entid(handle as u32), state)
    {
        return Some(steam_id);
    }

    // Priority 2: m_hOwnerEntity (rockets, flares, crossbow bolts, syringes, arrows, energy orbs)
    if let Some(prop) = props.iter().find(|p| p.identifier == OWNER)
        && let Ok(handle) = i64::try_from(&prop.value)
        && let Some(steam_id) = steam_id_from_entid(handle_to_entid(handle as u32), state)
    {
        return Some(steam_id);
    }

    None
}

fn steam_id_from_entid(entid: EntityId, state: &CheatAnalyserState) -> Option<SteamID> {
    if u32::from(entid) >= 2047 || u32::from(entid) == 0 {
        return None;
    }
    // Check direct Player match in state.players
    if let Some(player) = state.players.iter().find(|p| p.entity == entid)
        && let Some(sid) = player_id(player)
    {
        return Some(sid);
    }
    // Check entid_to_userid -> userid_to_id64 mapping
    if let Some(user_id) = state.get_userid_from_entid(entid)
        && let Some(sid) = state.get_steamid_from_userid(user_id)
    {
        return Some(sid);
    }
    None
}

fn is_accepted_projectile(class: &str) -> bool {
    matches!(
        class,
        "CTFProjectile_Rocket"
            | "CTFGrenadePipebombProjectile"
            | "CTFProjectile_Flare"
            | "CTFProjectile_BallOfFire"
            | "CTFProjectile_HealingBolt"
            | "CTFProjectile_Syringe"
            | "CTFProjectile_Arrow"
            | "CTFProjectile_EnergyBall"
            | "CTFProjectile_EnergyRing"
            | "CTFProjectile_MechanicalArmOrb"
    )
}

fn is_deflected_or_touched(props: &[tf_demo_parser::demo::sendprop::SendProp]) -> bool {
    let deflected = projectile_integer(props, &[("DT_TFBaseRocket", "m_iDeflected"), ("DT_TFWeaponBaseGrenadeProj", "m_iDeflected")]);
    if deflected.is_some_and(|d| d > 0) {
        return true;
    }

    let touched = projectile_integer(props, &[("DT_TFProjectile_Pipebomb", "m_bTouched")]);
    touched.is_some_and(|t| t > 0)
}

fn extract_initial_velocity(class: &str, props: &[tf_demo_parser::demo::sendprop::SendProp]) -> Option<Vec3> {
    if class == "CTFProjectile_MechanicalArmOrb" || class == "CTFProjectile_BallOfFire" {
        let ang_rotation = projectile_vector(props, &[("DT_TFBaseRocket", "m_angRotation"), ("DT_BaseEntity", "m_angRotation")]);
        if let Some(rot) = ang_rotation {
            let speed = if class == "CTFProjectile_BallOfFire" { 3000.0 } else { 1100.0 };
            // TODO: check that x & y are not swapped
            let dir = ViewAngle { yaw: rot.y, pitch: rot.x }.to_unit_vec();
            return Some(Vec3 {
                x: dir.x * speed,
                y: dir.y * speed,
                z: dir.z * speed,
            });
        }
    }

    let vel = projectile_vector(
        props,
        &[("DT_TFBaseRocket", "m_vInitialVelocity"), ("DT_TFWeaponBaseGrenadeProj", "m_vInitialVelocity")],
    );
    if let Some(v) = vel
        && v.length_square() > 1.0
    {
        return Some(v);
    }

    // Fallback to m_angRotation if initial velocity is missing/zero
    let ang_rotation = projectile_vector(props, &[("DT_TFBaseRocket", "m_angRotation"), ("DT_BaseEntity", "m_angRotation")]);
    if let Some(rot) = ang_rotation {
        // TODO: check that x & y are not swapped
        let dir = ViewAngle { yaw: rot.y, pitch: rot.x }.to_unit_vec();
        return Some(Vec3 {
            x: dir.x * 1100.0,
            y: dir.y * 1100.0,
            z: dir.z * 1100.0,
        });
    }

    None
}

fn projectile_vector(props: &[tf_demo_parser::demo::sendprop::SendProp], identifiers: &[(&str, &str)]) -> Option<Vec3> {
    identifiers.iter().find_map(|(table, name)| {
        let identifier = SendPropIdentifier::new(table, name);
        props
            .iter()
            .find(|prop| prop.identifier == identifier)
            .and_then(|prop| Vec3::try_from(&prop.value).ok())
    })
}

fn projectile_integer(props: &[tf_demo_parser::demo::sendprop::SendProp], identifiers: &[(&str, &str)]) -> Option<i64> {
    identifiers.iter().find_map(|(table, name)| {
        let identifier = SendPropIdentifier::new(table, name);
        props
            .iter()
            .find(|prop| prop.identifier == identifier)
            .and_then(|prop| i64::try_from(&prop.value).ok())
    })
}

fn player_id(player: &Player) -> Option<SteamID> {
    let info = player.info.as_ref()?;
    SteamID::from_steam3(&info.steam_id).ok()
}

fn normalized(vector: Vec3) -> Option<Vec3> {
    let length = vector.length();
    if !length.is_finite() || length < 100.0 * f32::EPSILON {
        return None;
    }
    Some(Vec3 {
        x: vector.x / length,
        y: vector.y / length,
        z: vector.z / length,
    })
}

/// get the Vec3 that is ViewAngle.pitch + 90°
// TODO: maybe move to method inside `ViewAngle` or remove.
fn angles_to_up_vector(viewangle: ViewAngle) -> Vec3 {
    let yaw_rad = viewangle.yaw.to_radians();
    let pitch_rad = viewangle.pitch.to_radians();
    Vec3 {
        x: pitch_rad.sin() * yaw_rad.cos(),
        y: pitch_rad.sin() * yaw_rad.sin(),
        z: pitch_rad.cos(),
    }
}

impl From<TentativeDetection> for Detection {
    fn from(value: TentativeDetection) -> Self {
        let data = json!({
            "class": &value.class.to_string(),
            "weapon": &value.weapon_name,
            "projectile": &value.projectile_name,
            "projectile_entity": &value.projectile_entity,
            "projectile_serial": &value.projectile_serial,
            "origin": &value.origin,
            "initial_velocity": &value.initial_velocity,
            "launch_angle": &value.launch_angle,
            "view_angle": &value.view_angle,
            "view_tick": &value.view_tick,
            "raw_angle_delta": &value.raw_angle_delta,
            "compensated_angle_delta": &value.compensated_angle_delta,
            "view_angle_delta": &value.view_angle_delta,
            "owner_distance": &value.owner_distance,
            "pipe_type": &value.pipe_type,
            "sample_count": &value.sample_count,
        });
        Detection {
            tick: value.tick,
            algorithm: Psilent5::ALGORITHM_NAME,
            player: value.player,
            data,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_forward_up_orthogonality() {
        let fwd = ViewAngle { yaw: 35.0, pitch: 20.0 }.to_unit_vec();
        let up = angles_to_up_vector(ViewAngle { yaw: 35.0, pitch: 20.0 });
        let d = fwd.dot(up);
        assert!(d.abs() < 1e-5, "forward and up must be orthogonal");
    }
}
