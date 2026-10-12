// Written by Teltta
// PERF: check why this algorithm eats performance and if it can be improved, fix that
use crate::{
    base::cheat_analyser_base::CheatAnalyserState,
    lib::{
        algorithm::{CheatAlgorithm, Detection},
        parameters::AlgorithmParameters,
    },
    make_algorithm_params,
    util::{
        helpers::{
            KeyValueVec, ViewAngle,
            player_finders::{players_alive_and_in_pvs, valid_players},
        },
        nocrex::jankguard::JankGuard,
    },
};
use anyhow::{Context, Error};
use serde_json::json;
use steamid_ng::SteamID;
use tf_demo_parser::ParserState;

#[derive(Debug, Clone, Default)]
pub struct AngleHistory {
    ticks: Vec<KeyValueVec<SteamID, ViewAngle>>,

    jg: JankGuard,
    params: Parameters,
    detections: Vec<Detection>,
}

#[derive(Debug, Clone, Copy)]
pub struct Parameters {
    pub tick_window: usize,
    pub max_delta_first_third: f32,
    pub min_delta_second_third: f32,
}

impl Default for Parameters {
    fn default() -> Self {
        Self {
            tick_window: 4,
            max_delta_first_third: 0.5,
            min_delta_second_third: 10.0,
        }
    }
}

impl CheatAlgorithm for AngleHistory {
    fn enabled_by_default(&self) -> bool {
        true
    }

    fn algorithm_name(&self) -> &str {
        "teltta/angle_history"
    }

    fn on_tick(&mut self, state: &CheatAnalyserState, _: &ParserState) -> Result<Vec<Detection>, Error> {
        self.jg.on_tick(state).context(format!("jankguard errored in on_tick with state {:?}", state))?;
        let ticknum = u32::from(state.tick);
        let players = &state.players;

        let tick_window = self.params.tick_window;
        let max_delta_first_third = self.params.max_delta_first_third;
        let min_delta_second_third = self.params.min_delta_second_third;

        self.ticks.insert(0, Default::default());
        self.ticks.truncate(tick_window);

        for (player, _info, steam_id) in valid_players(players_alive_and_in_pvs(players.iter())) {
            let ticks_since_event = self.jg.teleported(&steam_id, ticknum).min(self.jg.spawned(&steam_id, ticknum));

            if ticks_since_event < 60 {
                if ticks_since_event == 0 {
                    self.detections.retain(|det| det.player != steam_id || (ticknum - det.tick) > 60);
                }
                continue;
            }

            _ = self
                .ticks
                .get_mut(0)
                .expect("the first tick was inserted earlier this function and must exist")
                .insert(steam_id, player.view_angle);

            let current_angle = player.view_angle;

            let Some((match_index, delta_one)) = self
                .ticks
                .iter()
                .enumerate()
                .skip(1)
                .filter_map(|(i, past_instance)| {
                    past_instance
                        .get(&steam_id)
                        .map(|past_viewangle| (i, current_angle.world_angle_delta(*past_viewangle)))
                })
                .find(|(_, delta)| *delta < max_delta_first_third)
            else {
                continue;
            };

            // TODO: understand why the following does what it does, or change it
            let mid = match_index.div_ceil(2);
            let mut mids = vec![mid];

            if (match_index - 1) % 2 != 0 {
                mids.push(mid + 1);
            }

            for (m, mid_angle) in mids
                .iter()
                .filter_map(|m| self.ticks.get(*m).and_then(|map| map.get(&steam_id)).map(|x| (m, *x)))
            {
                let mid_delta = current_angle.world_angle_delta(mid_angle);

                if mid_delta > min_delta_second_third && self.jg.fired(&steam_id, ticknum) <= (match_index as u32 + 5) {
                    self.detections.push(Detection {
                        tick: ticknum,
                        algorithm: self.algorithm_name().to_string(),
                        player: steam_id,
                        data: json!({
                            // TODO: check if `current_angle` needs to be present twice
                            "angle_current": current_angle,
                            "angle_middle": mid_angle,
                            "angle_trigger": current_angle,
                            "delta_1_3": delta_one,
                            "delta_2_3": mid_delta,
                            "match_index": match_index,
                            "middle_indices": mids,
                            "middle_trigger": m,
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

    fn params(&mut self) -> Option<AlgorithmParameters<'_>> {
        let params = make_algorithm_params!(self.params; [
            tick_window, max_delta_first_third, min_delta_second_third,
        ]);
        Some(params)
    }
}
