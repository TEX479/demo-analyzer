// Written by Nocrex, Patched for Command Batching by Ciam

use std::collections::HashMap;

use crate::{
    base::cheat_analyser_base::{CheatAnalyserState, PlayerState},
    util::{
        helpers::{ViewAngle, viewangle_delta},
        nocrex::jankguard::JankGuard,
    },
};

use crate::lib::algorithm::{CheatAlgorithm, Detection};
use crate::lib::parameters::{Parameter, Parameters, get_parameter_value};

use anyhow::{Context, Error};
use serde_json::json;
use steamid_ng::SteamID;
use tf_demo_parser::ParserState;

#[derive(Default)]
pub struct AngleRepeat {
    // TODO: replace with custom history struct
    ticks: Vec<(u32, HashMap<SteamID, ViewAngle>)>,

    jg: JankGuard,
    params: Parameters,
    detections: Vec<Detection>,
}

impl AngleRepeat {
    pub fn new() -> Self {
        Self {
            params: HashMap::from([
                ("min_angle_diff_ratio".to_string(), Parameter::Float(3.0)),
                ("min_first_second_angle_delta".to_string(), Parameter::Float(0.0)),
                ("max_first_third_angle_delta".to_string(), Parameter::Float(0.028)),
            ]),
            ..Default::default()
        }
    }
}

impl CheatAlgorithm for AngleRepeat {
    fn enabled_by_default(&self) -> bool {
        true
    }

    fn algorithm_name(&self) -> &str {
        "nocrex/angle_repeat"
    }

    fn on_tick(&mut self, state: &CheatAnalyserState, _: &ParserState) -> Result<Vec<Detection>, Error> {
        self.jg.on_tick(state).context(format!("jankguard errored in on_tick with state {:?}", state))?;
        let ticknum = u32::from(state.tick);
        let players = &state.players;

        self.ticks.insert(0, (ticknum, HashMap::new()));
        self.ticks.truncate(3);

        let min_angle_diff_ratio: f32 = get_parameter_value(&self.params, "min_angle_diff_ratio")?;
        let min_first_second_angle_delta: f32 = get_parameter_value(&self.params, "min_first_second_angle_delta")?;
        let max_first_third_angle_delta: f32 = get_parameter_value(&self.params, "max_first_third_angle_delta")?;

        for player in players
            .iter()
            .filter(|p| p.in_pvs && p.state == PlayerState::Alive && p.info.as_ref().is_some_and(|info| info.steam_id != "BOT"))
        {
            let info = match &player.info {
                Some(info) => info,
                None => continue,
            };

            let steam_id = SteamID::from_steam3(&info.steam_id).context(format!("invalid steam_id3: {:?}", info.steam_id))?;

            let prev_data = self.ticks.get(1).and_then(|(t, m)| m.get(&steam_id).map(|p| (*t, *p)));
            let second_prev_data = self.ticks.get(2).and_then(|(t, m)| m.get(&steam_id).map(|p| (*t, *p)));

            let ticks_since_event = self.jg.teleported(&steam_id, ticknum).min(self.jg.spawned(&steam_id, ticknum));

            if ticks_since_event < 60 {
                // Ignore detections +-60 ticks from a teleport or spawn event
                if ticks_since_event == 0 {
                    self.detections.retain(|det| det.player != steam_id || (ticknum - det.tick) > 60);
                }
                continue;
            }

            let third_angle = ViewAngle::from(player);
            self.ticks
                .get_mut(0)
                .expect("the first tick was inserted at the start of this function")
                .1
                .insert(steam_id, third_angle); // Store angle for this tick for next ticks

            if let (Some((second_t, second_angle)), Some((first_t, first_angle))) = (prev_data, second_prev_data) {
                let first_second_delta = calc_real_delta(first_t, first_angle, second_t, second_angle);
                let first_third_delta = calc_real_delta(first_t, first_angle, ticknum, third_angle);

                if first_second_delta < min_first_second_angle_delta {
                    // Ignore players with only a tiny adjustment in second angle
                    continue;
                }

                let ratio = first_second_delta / first_third_delta.max(1.0);

                if first_third_delta <= max_first_third_angle_delta && ratio > min_angle_diff_ratio && self.jg.fired(&steam_id, ticknum) < 3 {
                    self.detections.push(Detection {
                        tick: ticknum,
                        algorithm: self.algorithm_name().to_string(),
                        player: steam_id,
                        data: json!({
                            "angle_1": first_angle,
                            "angle_2": second_angle,
                            "angle_3": third_angle,
                            "1_3_delta": first_third_delta,
                            "1_2_delta": first_second_delta,
                            "ratio": ratio,
                        }),
                    });
                }
            }
        }
        Ok(vec![])
    }

    fn handled_messages(&self) -> Result<Vec<tf_demo_parser::MessageType>, bool> {
        self.jg.handled_messages()
    }

    fn on_message(
        &mut self,
        message: &tf_demo_parser::demo::message::Message,
        state: &CheatAnalyserState,
        parser_state: &ParserState,
        tick: tf_demo_parser::demo::data::DemoTick,
    ) -> Result<Vec<Detection>, Error> {
        self.jg.on_message(message, state, parser_state, tick);
        Ok(vec![])
    }

    fn finish(&mut self) -> Result<Vec<Detection>, Error> {
        Ok(self.detections.clone())
    }

    fn params(&mut self) -> Option<&mut Parameters> {
        Some(&mut self.params)
    }
}

fn calc_real_delta(tick_old: u32, angle_old: ViewAngle, tick_new: u32, angle_new: ViewAngle) -> f32 {
    let tick_delta = tick_new.saturating_sub(tick_old);

    let (va_real, pa_real) = viewangle_delta(angle_new, angle_old, tick_delta);

    (va_real * va_real + pa_real * pa_real).sqrt()
}
