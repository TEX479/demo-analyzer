// TODO: refactor more
use std::collections::HashMap;

use anyhow::Error;
use serde_json::json;
use steamid_ng::SteamID;
use tf_demo_parser::ParserState;
use tf_demo_parser::demo::parser::analyser::Class;

use crate::base::cheat_analyser_base::CheatAnalyserState;
use crate::lib::algorithm::{CheatAlgorithm, Detection};
use crate::lib::parameters::AlgorithmParameters;
use crate::make_algorithm_params;
use crate::util::helpers::Vec3;
use crate::util::helpers::player_finders::{players_alive_and_in_pvs, valid_players};
use crate::util::nocrex::jankguard::JankGuard;

const TICK_INTERVAL: f32 = 0.015; // 66.67 Hz standard Source engine tick interval

#[derive(Default, Clone, Debug)]
struct PlayerCrouchSpeedState {
    prev_pos: Option<Vec3>,
    last_sample_pos: Option<Vec3>,
    last_sample_tick: u32,
    current_speed_hu_s: f32,
    consecutive_ticks: u32,
    pvs_ticks: u32,
    last_detection_tick: Option<u32>,
}

#[derive(Debug, Clone, Default)]
pub struct CrouchSpeed {
    params: Parameters,
    player_states: HashMap<SteamID, PlayerCrouchSpeedState>,
}

#[derive(Debug, Clone, Copy)]
pub struct Parameters {
    /// absolute minimum speed in HammerUnits per seconds (`HU/s`)
    /// Absolute max legitimate crouch speed in TF2 (BFB Scout at 520 boost) is 520/3 = 173.33 HU/s.
    /// All normal classes crouch at 77-133 HU/s. Cheaters with DuckSpeed move at 200-350+ HU/s.
    pub min_speed: f32,
    /// Minimum fraction of class standing base speed (e.g. 0.55 * 400 = 220 HU/s for Scout)
    pub speed_ratio: f32,
    /// Minimum consecutive on-ground crouched fast-moving ticks required to flag (45 ticks ~= 0.68s).
    /// C-tap is 1-2 ticks; rocket jump landings slide for <20 ticks; running into duck decelerates in <20 ticks.
    pub min_consecutive_ticks: usize,
    /// Vertical speed limit: downward slides/falls on steep slopes are ignored
    pub max_vertical_speed: f32,
    /// Ticks between detections for the same player
    pub debounce_ticks: usize,
}

impl Default for Parameters {
    fn default() -> Self {
        Self {
            min_speed: 180.0,
            speed_ratio: 0.55,
            min_consecutive_ticks: 45,
            max_vertical_speed: 100.0,
            debounce_ticks: 66,
        }
    }
}

impl CrouchSpeed {
    const ALGORITHM_NAME: &'static str = "fidoo/crouchspeed";

    pub fn new() -> Self {
        Self {
            player_states: HashMap::with_capacity(32),
            ..Default::default()
        }
    }

    fn get_class_base_speed(class: Class) -> f32 {
        match class {
            Class::Scout => 400.0,
            Class::Soldier => 240.0,
            Class::Pyro => 300.0,
            Class::Demoman => 280.0,
            Class::Heavy => 230.0,
            Class::Engineer => 300.0,
            Class::Medic => 320.0,
            Class::Sniper => 300.0,
            Class::Spy => 320.0,
            Class::Other => 300.0,
        }
    }
}

impl CheatAlgorithm for CrouchSpeed {
    fn enabled_by_default(&self) -> bool {
        true
    }

    fn algorithm_name(&self) -> &str {
        Self::ALGORITHM_NAME
    }

    fn params(&mut self) -> Option<AlgorithmParameters<'_>> {
        let params = make_algorithm_params!(self.params; [
            min_speed, speed_ratio, min_consecutive_ticks, max_vertical_speed, debounce_ticks
        ]);
        Some(params)
    }

    fn init(&mut self) -> Result<(), Error> {
        self.player_states.clear();
        Ok(())
    }

    fn on_tick(&mut self, state: &CheatAnalyserState, _parser_state: &ParserState, jg: &JankGuard) -> Result<Vec<Detection>, Error> {
        let ticknum = u32::from(state.tick);

        let min_speed = self.params.min_speed;
        let speed_ratio = self.params.speed_ratio;
        let min_consecutive_ticks = self.params.min_consecutive_ticks;
        let max_vertical_speed = self.params.max_vertical_speed;
        let debounce_ticks = self.params.debounce_ticks;

        let mut detections = Vec::new();

        for (player, _info, steam_id) in valid_players(players_alive_and_in_pvs(state.players.iter())) {
            // Spawn and teleport grace window (60 ticks)
            let ticks_since_event = jg.teleported(&steam_id, ticknum).min(jg.spawned(&steam_id, ticknum));
            if ticks_since_event < 60 {
                self.player_states.remove(&steam_id);
                continue;
            }

            // Exclude non-walking contexts:
            // 1. In water / swimming
            if player.is_in_water() {
                if let Some(pstate) = self.player_states.get_mut(&steam_id) {
                    pstate.consecutive_ticks = 0;
                }
                continue;
            }

            // 2. Taunting (e.g. Conga / Kazotsky)
            if player.is_taunting() {
                if let Some(pstate) = self.player_states.get_mut(&steam_id) {
                    pstate.consecutive_ticks = 0;
                }
                continue;
            }

            // 3. Demoman shield charge (cond 17, speeds up to 750 HU/s)
            if (player.cond & (1 << 17)) != 0 {
                if let Some(pstate) = self.player_states.get_mut(&steam_id) {
                    pstate.consecutive_ticks = 0;
                }
                continue;
            }

            // 4. Halloween bumper kart (cond 82: cond_ex2 bit 18)
            if (player.cond_ex2 & (1 << (82 - 64))) != 0 {
                if let Some(pstate) = self.player_states.get_mut(&steam_id) {
                    pstate.consecutive_ticks = 0;
                }
                continue;
            }

            let pstate = self.player_states.entry(steam_id).or_default();
            pstate.pvs_ticks += 1;

            // Wait at least 4 ticks after entering PVS to establish valid position deltas
            if pstate.pvs_ticks < 4 {
                pstate.prev_pos = Some(player.position);
                pstate.last_sample_pos = Some(player.position);
                pstate.last_sample_tick = ticknum;
                continue;
            }

            let (dx, dy, dz) = if let Some(prev) = pstate.prev_pos {
                (player.position.x - prev.x, player.position.y - prev.y, player.position.z - prev.z)
            } else {
                (0.0, 0.0, 0.0)
            };

            let moved = (dx * dx + dy * dy) > 0.001 || dz.abs() > 0.001;

            if moved {
                let tick_delta = if pstate.last_sample_tick > 0 {
                    (ticknum - pstate.last_sample_tick).max(1)
                } else {
                    1
                };

                if let Some(last_sample) = pstate.last_sample_pos {
                    let sample_dx = player.position.x - last_sample.x;
                    let sample_dy = player.position.y - last_sample.y;
                    let dist_2d = (sample_dx * sample_dx + sample_dy * sample_dy).sqrt();
                    let dt = tick_delta as f32 * TICK_INTERVAL;
                    pstate.current_speed_hu_s = dist_2d / dt;
                }

                pstate.last_sample_pos = Some(player.position);
                pstate.last_sample_tick = ticknum;
            } else if ticknum.saturating_sub(pstate.last_sample_tick) > 4 {
                // If no position packet has arrived for > 4 ticks (60ms), entity is stationary
                pstate.current_speed_hu_s = 0.0;
            }

            pstate.prev_pos = Some(player.position);

            let is_on_ground = player.is_on_ground();
            let is_ducking = player.is_ducking();
            let current_speed = pstate.current_speed_hu_s;

            // Effective threshold: minimum 180 HU/s or 55% of class base standing speed
            let base_speed = Self::get_class_base_speed(player.class);
            let effective_threshold = min_speed.max(base_speed * speed_ratio);

            // Filter out steep downward slopes/slides/falls (vz < -100 HU/s)
            let is_downward_slide = dz < -(max_vertical_speed * TICK_INTERVAL);

            if is_on_ground && is_ducking && current_speed >= effective_threshold && !is_downward_slide {
                pstate.consecutive_ticks += 1;

                if pstate.consecutive_ticks >= min_consecutive_ticks as u32 {
                    let should_emit = match pstate.last_detection_tick {
                        Some(last_tick) => ticknum.saturating_sub(last_tick) >= debounce_ticks as u32,
                        None => true,
                    };

                    if should_emit {
                        pstate.last_detection_tick = Some(ticknum);
                        detections.push(Detection {
                            tick: ticknum,
                            algorithm: Self::ALGORITHM_NAME.to_string(),
                            player: steam_id,
                            data: json!({
                                "speed": (current_speed * 10.0).round() / 10.0,
                                "threshold": effective_threshold,
                                "consecutive_ticks": pstate.consecutive_ticks,
                                "class": player.class_name(),
                            }),
                        });
                    }
                }
            } else {
                pstate.consecutive_ticks = 0;
            }
        }

        Ok(detections)
    }

    fn finish(&mut self) -> Result<Vec<Detection>, Error> {
        Ok(vec![])
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::base::cheat_analyser_base::Player;
    use crate::base::cheat_analyser_base::PlayerState;
    use crate::base::cheat_analyser_base::Team;
    use tf_demo_parser::demo::parser::analyser::UserInfo;

    fn make_test_player(steam_id: SteamID, class: Class) -> Player {
        Player {
            in_pvs: true,
            state: PlayerState::Alive,
            class,
            flags: 1 | (1 << 1), // ONGROUND (1) and DUCKING (2)
            position: Vec3 { x: 0.0, y: 0.0, z: 0.0 },
            info: Some(UserInfo {
                classes: Default::default(),
                name: "TestSubject".to_string(),
                user_id: 1u32.into(),
                steam_id: steam_id.steam3(),
                entity_id: 1u32.into(),
                team: Team::Red,
            }),
            ..Default::default()
        }
    }

    #[test]
    fn test_clean_c_tap_does_not_flag() {
        // C-tap: Soldier is running on ground, ducks for 2 ticks, then jumps into air
        let mut algo = CrouchSpeed::new();
        let mut jg = JankGuard::default();
        let mut state = CheatAnalyserState::default();
        let parser_state = ParserState::new(1, |_| true, false);

        let sid = SteamID::from(76561198000000001);
        let mut player = make_test_player(sid, Class::Soldier);

        // Tick 100-115: Running standing on ground
        player.flags = 1; // ONGROUND, NOT ducking
        for t in 100..=115 {
            state.tick = t.into();
            player.position.x += 3.6; // 240 HU/s
            state.players = vec![player.clone()];
            jg.on_tick(&state).unwrap();
            let dets = algo.on_tick(&state, &parser_state, &jg).unwrap();
            assert!(dets.is_empty());
        }

        // Tick 116-117: C-tap! Ducking on ground for 2 ticks
        player.flags = 1 | (1 << 1); // ONGROUND and DUCKING
        for t in 116..=117 {
            state.tick = t.into();
            player.position.x += 3.6;
            state.players = vec![player.clone()];
            jg.on_tick(&state).unwrap();
            let dets = algo.on_tick(&state, &parser_state, &jg).unwrap();
            assert!(dets.is_empty(), "C-tap must not trigger detection!");
        }

        // Tick 118-140: Airborne blast jump (NOT on ground)
        player.flags = 1 << 1; // In air, ducking
        for t in 118..=140 {
            state.tick = t.into();
            player.position.x += 12.0; // 800 HU/s
            player.position.z += 8.0;
            state.players = vec![player.clone()];
            jg.on_tick(&state).unwrap();
            let dets = algo.on_tick(&state, &parser_state, &jg).unwrap();
            assert!(dets.is_empty());
        }
    }

    #[test]
    fn test_normal_crouch_walk_does_not_flag() {
        // Normal crouch walk: Pyro walking at 100 HU/s (below threshold of 180 HU/s)
        let mut algo = CrouchSpeed::new();
        let mut jg = JankGuard::default();
        let mut state = CheatAnalyserState::default();
        let parser_state = ParserState::new(1, |_| true, false);

        let sid = SteamID::from(76561198000000002);
        let mut player = make_test_player(sid, Class::Pyro);

        for t in 100..=200 {
            state.tick = t.into();
            player.position.x += 1.5; // 100 HU/s (normal crouch)
            state.players = vec![player.clone()];
            jg.on_tick(&state).unwrap();
            let dets = algo.on_tick(&state, &parser_state, &jg).unwrap();
            assert!(dets.is_empty(), "Legitimate crouch walk must never flag!");
        }
    }

    // FIX: test fails
    #[test]
    fn test_crouchspeed_cheater_triggers_detection() {
        // Cheater with DuckSpeed: Soldier moving at 220 HU/s while crouched on ground for 50 ticks
        let mut algo = CrouchSpeed::new();
        let mut jg = JankGuard::default();
        let mut state = CheatAnalyserState::default();
        let parser_state = ParserState::new(1, |_| true, false);

        let sid = SteamID::from(76561198000000003);
        let mut player = make_test_player(sid, Class::Soldier);

        let mut triggered = false;
        for t in 100..=170 {
            state.tick = t.into();
            player.position.x += 3.3; // 220 HU/s (cheater duck speed for soldier)
            state.players = vec![player.clone()];
            jg.on_tick(&state).unwrap();
            let dets = algo.on_tick(&state, &parser_state, &jg).unwrap();
            if !dets.is_empty() {
                triggered = true;
                assert_eq!(dets[0].algorithm, "fidoo/crouchspeed");
                assert_eq!(dets[0].player, sid);
                break;
            }
        }
        assert!(triggered, "Crouchspeed cheater must be detected after min_consecutive_ticks!");
    }
}
