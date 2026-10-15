// refactor more
use std::collections::HashMap;

use anyhow::Error;
use serde_json::json;
use steamid_ng::SteamID;
use tf_demo_parser::demo::message::Message;
use tf_demo_parser::demo::message::packetentities::EntityId;
use tf_demo_parser::demo::sendprop::SendPropIdentifier;
use tf_demo_parser::{MessageType, ParserState};

use crate::base::cheat_analyser_base::CheatAnalyserState;
use crate::lib::algorithm::{CheatAlgorithm, Detection};
use crate::lib::parameters::AlgorithmParameters;
use crate::make_algorithm_params;
use crate::util::helpers::geometry::ViewAngleDelta;
use crate::util::helpers::player_finders::{players_alive_and_in_pvs, valid_players};
use crate::util::helpers::{KeyValueVec, ViewAngle, handle_to_entid};
use crate::util::nocrex::jankguard::{EVENT_TIMEOUT, JankGuard};

static MAX_VALID_SEEDS: usize = 16;

#[derive(Clone, Copy, Debug)]
struct WeaponSpreadInfo {
    category_id: u32,
    name: &'static str,
    spread_factor: f32,
    max_cone_degrees: f32,
}

fn get_weapon_spread_info(weapon_id: Option<u32>, weapon_name: &str, class_name: &str) -> Option<WeaponSpreadInfo> {
    if let Some(id) = weapon_id {
        match id {
            18 => {
                return Some(WeaponSpreadInfo {
                    category_id: 18,
                    name: "Minigun",
                    spread_factor: 0.030,
                    max_cone_degrees: 1.72,
                });
            }
            16 => {
                return Some(WeaponSpreadInfo {
                    category_id: 16,
                    name: "SMG",
                    spread_factor: 0.025,
                    max_cone_degrees: 1.43,
                });
            }
            22 => {
                return Some(WeaponSpreadInfo {
                    category_id: 22,
                    name: "Pistol",
                    spread_factor: 0.025,
                    max_cone_degrees: 1.43,
                });
            }
            24 => {
                return Some(WeaponSpreadInfo {
                    category_id: 24,
                    name: "Revolver",
                    spread_factor: 0.020,
                    max_cone_degrees: 1.15,
                });
            }
            13 | 10 | 11 | 9 | 19 | 12 => {
                return Some(WeaponSpreadInfo {
                    category_id: 13,
                    name: "Scattergun",
                    spread_factor: 0.040,
                    max_cone_degrees: 2.29,
                });
            }
            _ => {}
        }
    }

    if weapon_name.contains("Minigun")
        || weapon_name.contains("Tomislav")
        || weapon_name.contains("Brass Beast")
        || weapon_name.contains("Huo-Long")
        || weapon_name.contains("Deflector")
        || class_name == "CTFMinigun"
        || class_name == "heavy"
    {
        return Some(WeaponSpreadInfo {
            category_id: 18,
            name: "Minigun",
            spread_factor: 0.030,
            max_cone_degrees: 1.72,
        });
    }

    if weapon_name.contains("SMG") || weapon_name.contains("Cleaner's Carbine") || class_name == "CTFSMG" || class_name == "sniper" {
        return Some(WeaponSpreadInfo {
            category_id: 16,
            name: "SMG",
            spread_factor: 0.025,
            max_cone_degrees: 1.43,
        });
    }

    if weapon_name.contains("Pistol")
        || weapon_name.contains("Winger")
        || weapon_name.contains("Lugermorph")
        || weapon_name.contains("Pocket Pistol")
        || class_name.contains("Pistol")
    {
        return Some(WeaponSpreadInfo {
            category_id: 22,
            name: "Pistol",
            spread_factor: 0.025,
            max_cone_degrees: 1.43,
        });
    }

    if weapon_name.contains("Revolver")
        || weapon_name.contains("Ambassador")
        || weapon_name.contains("Big Kill")
        || weapon_name.contains("L'Etranger")
        || weapon_name.contains("Enforcer")
        || weapon_name.contains("Diamondback")
        || class_name == "CTFRevolver"
        || class_name == "spy"
    {
        return Some(WeaponSpreadInfo {
            category_id: 24,
            name: "Revolver",
            spread_factor: 0.020,
            max_cone_degrees: 1.15,
        });
    }

    if weapon_name.contains("Scattergun")
        || weapon_name.contains("Shortstop")
        || weapon_name.contains("Force-A-Nature")
        || weapon_name.contains("Soda Popper")
        || weapon_name.contains("Baby Face")
        || weapon_name.contains("Back Scatter")
        || weapon_name.contains("Shotgun")
        || class_name.contains("ScatterGun")
        || class_name.contains("Shotgun")
        || class_name == "scout"
    {
        return Some(WeaponSpreadInfo {
            category_id: 13,
            name: "Scattergun",
            spread_factor: 0.040,
            max_cone_degrees: 2.29,
        });
    }

    None
}

// Standard CUniformRandomStream PRNG
#[derive(Default)]
pub struct UniformRandomStream {
    m_idum: i32,
    m_iy: i32,
    m_iv: [i32; 32],
}

impl UniformRandomStream {
    pub fn set_seed(&mut self, i_seed: i32) {
        self.m_idum = if i_seed < 0 { i_seed } else { -i_seed };
        self.m_iy = 0;
    }

    pub fn generate_random_number(&mut self) -> i32 {
        const IA: i32 = 16807;
        const IM: i32 = 2147483647;
        const IQ: i32 = 127773;
        const IR: i32 = 2836;
        const NTAB: usize = 32;
        const NDIV: i32 = 1 + (IM - 1) / NTAB as i32;

        if self.m_idum <= 0 || self.m_iy == 0 {
            if -self.m_idum < 1 {
                self.m_idum = 1;
            } else {
                self.m_idum = -self.m_idum;
            }

            for j in (0..NTAB + 8).rev() {
                let k = self.m_idum / IQ;
                self.m_idum = IA.wrapping_mul(self.m_idum - k * IQ) - IR * k;
                if self.m_idum < 0 {
                    self.m_idum += IM;
                }
                if j < NTAB {
                    self.m_iv[j] = self.m_idum;
                }
            }
            self.m_iy = self.m_iv[0];
        }

        let k = self.m_idum / IQ;
        self.m_idum = IA.wrapping_mul(self.m_idum - k * IQ) - IR * k;
        if self.m_idum < 0 {
            self.m_idum += IM;
        }
        let j = (self.m_iy / NDIV) as usize;
        self.m_iy = self.m_iv[j];
        self.m_iv[j] = self.m_idum;

        self.m_iy
    }

    pub fn random_float(&mut self, fl_low: f32, fl_high: f32) -> f32 {
        const AM: f32 = 1.0 / 2147483647.0;
        let mut fl = AM * self.generate_random_number() as f32;
        if fl > 0.99999 {
            fl = 0.99999;
        }
        (fl * (fl_high - fl_low)) + fl_low
    }
}

fn compute_predicted_spread(seed: i32, spread_factor: f32) -> ViewAngleDelta {
    let mut rng = UniformRandomStream::default();
    rng.set_seed(seed & 255);
    let x = rng.random_float(-0.5, 0.5) + rng.random_float(-0.5, 0.5);
    let y = rng.random_float(-0.5, 0.5) + rng.random_float(-0.5, 0.5);

    let rad_to_deg = 180.0 / std::f32::consts::PI;
    let dx = x * spread_factor * rad_to_deg;
    let dy = y * spread_factor * rad_to_deg;
    // TODO: check if dx & dy need to be swapped
    ViewAngleDelta { yaw: dx, pitch: dy }
}

fn p_value_from_t(t: f64) -> f64 {
    let x = t.abs();
    const B1: f64 = 0.319381530;
    const B2: f64 = -0.356563782;
    const B3: f64 = 1.781477937;
    const B4: f64 = -1.821255978;
    const B5: f64 = 1.330274429;
    const P: f64 = 0.2316419;
    let k = 1.0 / (1.0 + P * x);
    let z = (-0.5 * x * x).exp() / (2.0 * std::f64::consts::PI).sqrt();
    let cdf_tail = z * (B1 * k + B2 * k.powi(2) + B3 * k.powi(3) + B4 * k.powi(4) + B5 * k.powi(5));
    (2.0 * cdf_tail).clamp(0.0, 1.0)
}

// TODO: refactor / check if this is already implemented somewhere
fn resolve_player_sid(player_raw: u32, state: &CheatAnalyserState) -> Option<SteamID> {
    let ent_ids = [
        EntityId::from(player_raw + 1),
        EntityId::from(player_raw),
        handle_to_entid(player_raw),
        handle_to_entid(player_raw + 1),
    ];
    for ent_id in ent_ids {
        if let Some(uid) = state.entid_to_userid.get(&ent_id)
            && let Some(sid) = state.userid_to_id64.get(uid)
        {
            return Some(*sid);
        }
        if let Some(player) = state.players.iter().find(|p| p.entity == ent_id)
            && let Some(info) = &player.info
            && let Ok(sid) = SteamID::from_steam3(&info.steam_id)
        {
            return Some(sid);
        }
    }
    None
}

pub type Tick = u32;

#[derive(Default, Clone)]
struct PlayerTracker {
    last_flag_tick: Tick,
    previous_ticks: KeyValueVec<Tick, PreviousTick>,
    current_weapon: Option<WeaponSpreadInfo>,
    previous_viewangle: Option<(ViewAngle, Tick)>,
    last_fire_event: Option<FireEvent>,
}

#[derive(Debug, Clone, Copy, Default)]
struct PreviousTick {
    /// ViewAngleDelta from previous to current tick
    delta: ViewAngleDelta,
    /// the seed from the FireEvent
    seed: Option<i32>,
    /// the spread from the FireEvent
    spread: Option<f32>,
}

#[derive(Debug, Clone, Copy, Default)]
struct FireEvent {
    pub tick: Tick,
    pub seed: Option<i32>,
    pub spread: Option<f32>,
    pub weapon_id: Option<u32>,
}

#[derive(Debug, Clone, Copy)]
pub struct Parameters {
    pub min_shots_evaluated: usize,
    pub min_seed_correlation: f32,
    pub min_variance_ratio: f32,
    pub max_tracking_variance: f32,
    pub spread_tolerance_margin: f32,
    pub min_micro_jitters: usize,
}

#[derive(Default)]
pub struct NoSpread {
    params: Parameters,
    // TODO: replace HashMap with Vec
    player_trackers: HashMap<SteamID, PlayerTracker>,
    detections: Vec<Detection>,
}

impl Default for Parameters {
    fn default() -> Self {
        Self {
            min_shots_evaluated: 8,
            min_seed_correlation: -0.85,
            min_variance_ratio: 4.5,
            max_tracking_variance: 0.5,
            spread_tolerance_margin: 0.5,
            min_micro_jitters: 3,
        }
    }
}

impl PlayerTracker {
    fn check_seed_correlation(&mut self, player_sid: SteamID, tick: u32, min_shots: usize, min_seed_correlation: f32) -> Option<Detection> {
        if self.previous_ticks.len() < min_shots {
            return None;
        }

        let weapon_info = self.current_weapon?;

        let valid_seeds: Vec<_> = self
            .previous_ticks
            .values()
            .iter()
            .rev()
            .filter_map(|s| s.seed.map(|seed| (s.delta, seed, s.spread)))
            .take(MAX_VALID_SEEDS)
            .collect();

        if valid_seeds.len() >= min_shots {
            let mut deltas = Vec::new();
            let mut preds = Vec::new();
            for (delta, seed, spread) in &valid_seeds {
                let spread_factor = spread.unwrap_or(weapon_info.spread_factor);
                let pred = compute_predicted_spread(*seed, spread_factor);
                deltas.push(*delta);
                preds.push(pred);
            }

            let multiplier = 1.0 / (deltas.len().max(1)) as f32;
            let mean_delta = deltas.iter().fold(ViewAngleDelta::default(), |acc, &va| acc + va) * multiplier;
            let mean_predicion = preds.iter().fold(ViewAngleDelta::default(), |acc, &va| acc + va) * multiplier;

            let mut cov = 0.0f32;
            let mut var_d = 0.0f32;
            let mut var_p = 0.0f32;
            let mut total_residual = 0.0f32;

            for (&delta, &pred) in deltas.iter().zip(preds.iter()) {
                let d = delta - mean_delta;
                let p = pred - mean_predicion;

                cov += d.dot(p);
                var_d += d.dot(d);
                var_p += p.dot(p);

                let res = delta + pred;
                total_residual += res.magnitude();
            }

            let denom = (var_d * var_p).sqrt();
            let r = if denom > 1e-7 { cov / denom } else { 0.0 };
            let mean_residual = total_residual * multiplier;

            let df = deltas.len() as f64 - 2_f64;
            let r_f64 = r as f64;
            let t_stat = r_f64 * (df / (1.0 - r_f64 * r_f64).max(1e-7)).sqrt();
            let p_val = p_value_from_t(t_stat);

            if r <= min_seed_correlation && p_val < 0.001 {
                self.last_flag_tick = tick;
                return Some(Detection {
                    tick,
                    algorithm: NoSpread::ALGORITHM_NAME,
                    player: player_sid,
                    data: json!({
                        "type": "SeedCorrelation",
                        "weapon_id": weapon_info.category_id,
                        "weapon_name": weapon_info.name,
                        "shot_count": deltas.len(),
                        "correlation_coefficient": (r * 1000.0).round() / 1000.0,
                        "p_value": (p_val * 100000.0).round() / 100000.0,
                        "mean_residual_degrees": (mean_residual * 1000.0).round() / 1000.0,
                    }),
                });
            }
        }
        None
    }
}

impl NoSpread {
    const ALGORITHM_NAME: &'static str = "fidoo/nospread";

    pub fn new() -> Self {
        Self::default()
    }
}

impl CheatAlgorithm for NoSpread {
    fn enabled_by_default(&self) -> bool {
        true
    }

    fn algorithm_name(&self) -> &'static str {
        Self::ALGORITHM_NAME
    }

    fn params(&mut self) -> Option<AlgorithmParameters<'_>> {
        let params = make_algorithm_params!(
            self.params; [
                min_shots_evaluated,
                min_seed_correlation,
                min_variance_ratio,
                max_tracking_variance,
                spread_tolerance_margin,
                min_micro_jitters
            ]
        );
        Some(params)
    }

    fn handled_messages(&self) -> Result<Vec<MessageType>, bool> {
        Ok(vec![MessageType::TempEntities, MessageType::GameEvent])
    }

    fn on_message(
        &mut self,
        message: &Message,
        state: &CheatAnalyserState,
        parser_state: &ParserState,
        _jg: &JankGuard,
        tick: tf_demo_parser::demo::data::DemoTick,
    ) -> Result<Vec<Detection>, Error> {
        if let Message::TempEntities(msg) = message {
            for event in &msg.events {
                let class = &parser_state.server_classes[usize::from(event.class_id)].name;
                if class.as_str() == "CTEFireBullets" {
                    const BULLETS_PLAYER: SendPropIdentifier = SendPropIdentifier::new("DT_TEFireBullets", "m_iPlayer");
                    const BULLETS_SEED: SendPropIdentifier = SendPropIdentifier::new("DT_TEFireBullets", "m_iSeed");
                    const BULLETS_SPREAD: SendPropIdentifier = SendPropIdentifier::new("DT_TEFireBullets", "m_flSpread");
                    const BULLETS_WEAPON: SendPropIdentifier = SendPropIdentifier::new("DT_TEFireBullets", "m_iWeaponID");

                    let mut player_opt = None;
                    let mut seed_opt = None;
                    let mut spread_opt = None;
                    let mut weapon_opt = None;

                    for prop in &event.props {
                        if prop.identifier == BULLETS_PLAYER {
                            if let Ok(id) = i64::try_from(&prop.value) {
                                player_opt = Some(id as u32);
                            }
                        } else if prop.identifier == BULLETS_SEED {
                            if let Ok(seed) = i64::try_from(&prop.value) {
                                seed_opt = Some(seed as i32);
                            }
                        } else if prop.identifier == BULLETS_SPREAD {
                            if let Ok(spread) = f32::try_from(&prop.value) {
                                spread_opt = Some(spread);
                            }
                        } else if prop.identifier == BULLETS_WEAPON
                            && let Ok(wep) = i64::try_from(&prop.value)
                        {
                            weapon_opt = Some(wep as u32);
                        }
                    }

                    if let Some(player_raw) = player_opt
                        && let Some(sid) = resolve_player_sid(player_raw, state)
                    {
                        let current_tick: u32 = tick.into();
                        self.player_trackers.entry(sid).or_default().last_fire_event = Some(FireEvent {
                            tick: current_tick,
                            seed: seed_opt,
                            spread: spread_opt,
                            weapon_id: weapon_opt,
                        });
                    }
                }
            }
        }

        Ok(vec![])
    }

    fn on_tick(&mut self, state: &CheatAnalyserState, _: &ParserState, jg: &JankGuard) -> Result<Vec<Detection>, Error> {
        let ticknum = u32::from(state.tick);

        self.params.min_shots_evaluated = self.params.min_shots_evaluated.clamp(4, MAX_VALID_SEEDS);
        let min_shots = self.params.min_shots_evaluated;
        let min_seed_correlation = self.params.min_seed_correlation;
        let spread_tolerance_margin = self.params.spread_tolerance_margin;

        let mut detections = Vec::new();

        for (player, _info, steam_id) in valid_players(players_alive_and_in_pvs(state.players.iter())) {
            let tracker = self.player_trackers.entry(steam_id).or_default();

            let ticks_since_event = jg.teleported(&steam_id, ticknum).min(jg.spawned(&steam_id, ticknum));
            if ticks_since_event < EVENT_TIMEOUT {
                if ticks_since_event == 0 {
                    self.detections.retain(|det| det.player != steam_id || (ticknum - det.tick) > EVENT_TIMEOUT);
                }
                tracker.previous_viewangle = Some((player.view_angle, ticknum));
                continue;
            }

            let Some((prev_angle, prev_tick)) = tracker.previous_viewangle else {
                tracker.previous_viewangle = Some((player.view_angle, ticknum));
                continue;
            };

            if ticknum == prev_tick + 1 {
                let va_delta = prev_angle.viewangle_delta(player.view_angle);
                tracker.previous_ticks.get_mut_or_insert_default(ticknum).delta = va_delta;
            }

            tracker.previous_viewangle = Some((player.view_angle, ticknum));

            // remove unnecessary ticks from player_history
            {
                let mut ticks_to_keep = Vec::new();
                let mut tick_to_check = ticknum;
                let mut ticks_checked = 0;
                while ticks_to_keep.len() < MAX_VALID_SEEDS + 4 && ticks_checked < tracker.previous_ticks.len() {
                    let Some(tick_data) = tracker.previous_ticks.get(&tick_to_check) else {
                        tick_to_check = match tick_to_check.checked_sub(1) {
                            Some(x) => x,
                            None => break,
                        };
                        continue;
                    };

                    if tick_data.seed.is_some() {
                        ticks_to_keep.push(tick_to_check);
                    }

                    ticks_checked += 1;
                    tick_to_check = match tick_to_check.checked_sub(1) {
                        Some(x) => x,
                        None => break,
                    };
                }
                tracker.previous_ticks.retain(|t, _| ticks_to_keep.contains(t));
            }

            // Process bullet fire events for this player
            if let Some(&FireEvent {
                tick: f_tick,
                seed,
                spread,
                weapon_id,
            }) = self.player_trackers.get(&steam_id).and_then(|x| x.last_fire_event.as_ref())
            {
                // Check if bullet was fired recently (within last 2 ticks)
                let fired_recently = ticknum >= f_tick && ticknum <= f_tick + 2;
                if !fired_recently {
                    continue;
                }
                let weapon_name = state.get_player_weapon(player);
                let class_name = player.class_name();
                let weapon_info = get_weapon_spread_info(weapon_id, weapon_name, class_name);

                let tracker = self.player_trackers.entry(steam_id).or_default();
                tracker.current_weapon = weapon_info;

                // Check single-tick flick and return reversal at f_tick or f_tick+1
                for t in f_tick..=f_tick + 1 {
                    let (Some(d1), Some(d2)) = (
                        tracker.previous_ticks.get(&t).map(|pt| pt.delta),
                        tracker.previous_ticks.get(&(t + 1)).map(|pt| pt.delta),
                    ) else {
                        continue;
                    };
                    let mag1 = d1.magnitude();
                    let mag2 = d2.magnitude();

                    let dot = d1.dot(d2);
                    let sum_mag = (d1 + d2).magnitude();

                    let max_cone = weapon_info.map_or(2.5, |w| w.max_cone_degrees) + spread_tolerance_margin;

                    if !(mag1 >= 0.70 && mag2 >= 0.70 && mag1 <= max_cone * 2.0 && dot < 0.0) {
                        continue;
                    }

                    let cos_angle = dot / (mag1 * mag2);
                    let cancel_ratio = sum_mag / mag1.max(mag2);

                    if !(cos_angle <= -0.65 && cancel_ratio <= 0.65) {
                        continue;
                    }

                    // Confirmed micro-reversal

                    // insert to previous_ticks
                    {
                        let pt = tracker.previous_ticks.get_mut_or_insert_default(t);
                        pt.delta = d1;
                        pt.seed = seed;
                        pt.spread = spread;
                    }

                    let recent_count = tracker
                        .previous_ticks
                        .iter()
                        .filter(|(pt_ticknum, _pt)| ticknum.saturating_sub(**pt_ticknum) <= 600)
                        .count();

                    let mean_mag: f32 = tracker
                        .previous_ticks
                        .iter()
                        .filter(|(pt_ticknum, _pt)| ticknum.saturating_sub(**pt_ticknum) <= 600)
                        .map(|(_, r)| r.delta.magnitude())
                        .sum::<f32>()
                        / recent_count.max(1) as f32;

                    let should_flag = recent_count >= self.params.min_micro_jitters && ticknum >= tracker.last_flag_tick + 60;
                    if should_flag {
                        tracker.last_flag_tick = ticknum;
                        let winfo = weapon_info.unwrap_or(WeaponSpreadInfo {
                            category_id: weapon_id.unwrap_or(0),
                            name: "SpreadWeapon",
                            spread_factor: 0.030,
                            max_cone_degrees: 1.72,
                        });

                        detections.push(Detection {
                            tick: ticknum,
                            algorithm: Self::ALGORITHM_NAME,
                            player: steam_id,
                            data: json!({
                                "type": "MicroJitterReversal",
                                "weapon_id": winfo.category_id,
                                "weapon_name": winfo.name,
                                "reversals_count": recent_count,
                                "mean_flick_magnitude": (mean_mag * 100.0).round() / 100.0,
                                "max_allowed_cone": (winfo.max_cone_degrees * 100.0).round() / 100.0,
                            }),
                        });
                    }

                    // Check Method A deterministic seed correlation
                    if let Some(det) = tracker.check_seed_correlation(steam_id, ticknum, min_shots, min_seed_correlation) {
                        detections.push(det);
                    }
                }
            }
        }

        let detections_final = self
            .detections
            .iter()
            .filter(|d| ticknum.saturating_sub(d.tick) >= EVENT_TIMEOUT)
            .cloned()
            .collect();
        self.detections.retain(|d| ticknum.saturating_sub(d.tick) < EVENT_TIMEOUT);

        // TODO: incorperate `self.params.min_detections` and improve this
        Ok(detections_final)
    }

    fn finish(&mut self) -> Result<Vec<Detection>, Error> {
        // TODO: check commit ba23116a9072 and maybe reimplement that functionality
        Ok(std::mem::take(&mut self.detections))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_min_detections_custom_param() {
        let sid1 = SteamID::from(11111u64);

        let mut algo = NoSpread::new();
        algo.params.min_micro_jitters = 1;

        algo.detections.push(Detection {
            tick: 100,
            algorithm: algo.algorithm_name(),
            player: sid1,
            data: json!({"type": "MicroJitterReversal"}),
        });

        let finished = algo.finish().unwrap();
        assert_eq!(finished.len(), 1);
        assert_eq!(finished[0].player, sid1);
    }
}
