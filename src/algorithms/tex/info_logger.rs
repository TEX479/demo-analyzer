use crate::make_algorithm_params;
use crate::util::nocrex::jankguard::JankGuard;
use crate::{lib::algorithm::CheatAlgorithm, util::helpers::KeyValueVec};
use itertools::Itertools;
use steamid_ng::SteamID;
use tf_demo_parser::demo::gamevent::GameEvent;
use tf_demo_parser::demo::message::{GameEventMessage, Message};
use tf_demo_parser::demo::parser::analyser::Class;

#[derive(Debug, Clone, Default)]
pub struct Logger {
    classes: KeyValueVec<SteamID, Class>,

    pub params: Parameters,
}

#[derive(Debug, Clone, Copy)]
pub struct Parameters {
    pub votes: bool,
    pub class_switches: bool,
}

impl Default for Parameters {
    fn default() -> Self {
        Self {
            votes: true,
            class_switches: false,
        }
    }
}

impl Logger {
    pub fn new() -> Self {
        Self::default()
    }
}

impl CheatAlgorithm for Logger {
    fn does_handle(&self, message_type: tf_demo_parser::MessageType) -> bool {
        matches!(message_type, tf_demo_parser::MessageType::GameEvent)
    }

    fn enabled_by_default(&self) -> bool {
        true
    }

    fn algorithm_name(&self) -> &'static str {
        "tex/info_logger"
    }

    fn params(&mut self) -> Option<crate::lib::parameters::AlgorithmParameters<'_>> {
        let params = make_algorithm_params!(self.params; [votes, class_switches]);
        Some(params)
    }

    fn on_message(
        &mut self,
        message: &tf_demo_parser::demo::message::Message,
        state: &crate::base::cheat_analyser_base::CheatAnalyserState,
        _parser_state: &tf_demo_parser::ParserState,
        _jg: &JankGuard,
        _tick: tf_demo_parser::demo::data::DemoTick,
    ) -> Result<Vec<crate::lib::algorithm::Detection>, anyhow::Error> {
        if let Message::GameEvent(GameEventMessage {
            event,
            event_type_id: _,
            event_type: _,
        }) = message
        {
            match event {
                GameEvent::PlayerSpawn(e) if self.params.class_switches => {
                    let Some(sid) = state.get_steamid_from_userid(e.user_id.into()) else {
                        return Ok(Vec::new());
                    };
                    let class = Class::new(e.class);
                    let should_log = self.update_class(sid, class);
                    if should_log {
                        let player_name = state
                            .get_player_name(sid)
                            .map(|x| x.to_owned())
                            .unwrap_or(format!("{}/{}", u64::from(sid), sid.steam3()));
                        tracing::info!("{} switched to {}", player_name, class);
                        // println!("{} switched to {}", player_name, class);
                    }
                }
                // TODO: maybe work out how this can be added
                // GameEvent::PlayerClass(e)
                GameEvent::PlayerChangeClass(e) if self.params.class_switches => {
                    let Some(sid) = state.get_steamid_from_userid(e.user_id.into()) else {
                        return Ok(Vec::new());
                    };
                    let class = Class::new(e.class);
                    let should_log = self.update_class(sid, class);
                    if should_log {
                        let player_name = state
                            .get_player_name(sid)
                            .map(|x| x.to_owned())
                            .unwrap_or(format!("{}/{}", u64::from(sid), sid.steam3()));
                        tracing::info!("{} switched to {}", player_name, class);
                        // println!("{} switched to {}", player_name, class);
                    }
                }
                GameEvent::VoteStarted(e) if self.params.votes => {
                    tracing::info!("New vote started -> {:?}", e);
                }
                GameEvent::VoteOptions(e) if self.params.votes => {
                    let mut vote_options_vec = Vec::new();
                    for i in 0..e.count {
                        let opt = match i {
                            0 => e.option_1.to_string(),
                            1 => e.option_2.to_string(),
                            2 => e.option_3.to_string(),
                            3 => e.option_4.to_string(),
                            4 => e.option_5.to_string(),
                            _ => String::new(),
                        };
                        vote_options_vec.push(opt);
                    }
                    let vote_options = vote_options_vec.into_iter().enumerate().map(|(i, s)| format!("{}: {:?}", i, s)).join(", ");
                    tracing::info!("vote options: {}", vote_options);
                }
                GameEvent::VoteCast(e) if self.params.votes => {
                    if let Some(player_identifier) = state
                        .players
                        .iter()
                        .find(|p| p.info.as_ref().is_some_and(|i| i.entity_id == e.entity_id))
                        .map(|player| player.info.as_ref().map(|i| i.name.clone()).unwrap_or(format!("{:?}", player.entity)))
                    {
                        tracing::info!("{} - {}", e.vote_option, player_identifier);
                    }
                }
                GameEvent::VotePassed(_) if self.params.votes => {
                    tracing::info!("vote success");
                }
                GameEvent::VoteFailed(_) if self.params.votes => {
                    tracing::info!("vote failed");
                }
                _ => (),
            }
        }
        Ok(Vec::new())
    }
}

impl Logger {
    /// updates the users class
    ///
    /// returns true if the user's class was not known or different
    fn update_class(&mut self, steam_id: SteamID, class: Class) -> bool {
        let is_same_class = match self.classes.insert(steam_id, class) {
            Some(class_old) => class_old == class,
            None => false,
        };
        !is_same_class
    }
}
