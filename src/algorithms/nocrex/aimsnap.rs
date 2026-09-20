// Written by Nocrex, Patched for Command Batching by Ciam

use crate::{
    base::cheat_analyser_base::CheatAnalyserState,
    lib::{
        algorithm::{CheatAlgorithm, Detection},
        parameters::{Parameter, Parameters, get_parameter_value},
    },
    util::{
        helpers::{ViewAngle, valid_players, viewangle_delta},
        nocrex::jankguard::JankGuard,
    },
};
use anyhow::{Context, Error};
use serde_json::json;
use std::{collections::HashMap, ops::Range};
use steamid_ng::SteamID;
use tf_demo_parser::ParserState;

#[derive(Default)]
pub struct AimSnap {
    // TODO: replace with custom history struct
    ticks: Vec<(u32, HashMap<SteamID, ViewAngle>)>,
    jg: JankGuard,
    params: Parameters,
    detections: Vec<Detection>,
}

impl AimSnap {
    pub fn new() -> Self {
        Self {
            params: HashMap::from([
                ("noise_min".to_string(), Parameter::Float(0.028)),
                ("noise_max".to_string(), Parameter::Float(0.99)),
                ("snap_threshold".to_string(), Parameter::Float(10.0)),
            ]),
            ..Default::default()
        }
    }
}

impl CheatAlgorithm for AimSnap {
    fn enabled_by_default(&self) -> bool {
        true
    }

    fn algorithm_name(&self) -> &str {
        "nocrex/aimsnap"
    }

    fn on_tick(&mut self, state: &CheatAnalyserState, _: &ParserState) -> Result<Vec<Detection>, Error> {
        self.jg.on_tick(state).context(format!("jankguard errored in on_tick with state {:?}", state))?;
        let ticknum = u32::from(state.tick);
        let players = &state.players;

        let noise_min: f32 = get_parameter_value(&self.params, "noise_min")?;
        let noise_max: f32 = get_parameter_value(&self.params, "noise_max")?;
        let snap_threshold: f32 = get_parameter_value(&self.params, "snap_threshold")?;

        let noise_range: Range<f32> = noise_min..noise_max;

        self.ticks.insert(0, (ticknum, HashMap::new()));
        self.ticks.truncate(5);

        for (player, _info, steam_id) in valid_players(players.iter()) {
            let ticks_since_event = self.jg.teleported(&steam_id, ticknum).min(self.jg.spawned(&steam_id, ticknum));

            if ticks_since_event < 60 {
                // Ignore detections +-60 ticks from a teleport or spawn event
                if ticks_since_event == 0 {
                    self.detections.retain(|det| det.player != steam_id || (ticknum - det.tick) > 60);
                }
                continue;
            }

            self.ticks
                .get_mut(0)
                .context("no first tick in AimSnap")?
                .1
                .insert(steam_id, ViewAngle::from(player)); // Store angle for this tick for next ticks

            let angle_history: Vec<_> = self.ticks.iter().filter_map(|(t, m)| m.get(&steam_id).map(|p| (*t, *p))).rev().collect();

            if angle_history.len() < self.ticks.len() {
                continue;
            }

            let mut deltas = Vec::new();
            for window in angle_history.windows(2) {
                let (t1, angle1) = window[0];
                let (t2, angle2) = window[1];

                let tick_delta = t2.saturating_sub(t1);

                let (va_delta_real, pa_delta_real) = viewangle_delta(angle2, angle1, tick_delta);

                let mag_delta = (va_delta_real * va_delta_real + pa_delta_real * pa_delta_real).sqrt();
                deltas.push(mag_delta);
            }

            if noise_range.contains(deltas.first().context("no first delta in AimSnap")?)
                && noise_range.contains(deltas.last().context("no last delta in AimSnap")?)
                && deltas.iter().filter(|&d| noise_range.contains(d)).count() == deltas.len() - 1
                && deltas.iter().filter(|&&d| d > snap_threshold).count() == 1
                && self.jg.fired(&steam_id, ticknum) < 5
            {
                self.detections.push(Detection {
                    tick: ticknum - 2,
                    algorithm: self.algorithm_name().to_string(),
                    player: steam_id,
                    data: json!({
                        "deltas": deltas
                    }),
                });
            }
        }
        Ok(vec![])
    }

    fn handled_messages(&self) -> Result<Vec<tf_demo_parser::MessageType>, bool> {
        self.jg.handled_messages()
    }

    fn finish(&mut self) -> Result<Vec<Detection>, Error> {
        Ok(self.detections.clone())
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

    fn params(&mut self) -> Option<&mut Parameters> {
        Some(&mut self.params)
    }
}
