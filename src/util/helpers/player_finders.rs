use crate::base::cheat_analyser_base::{Player, PlayerState};
use steamid_ng::SteamID;
use tf_demo_parser::demo::parser::analyser::{UserId, UserInfo};

pub fn valid_players<'a>(players: impl Iterator<Item = &'a Player>) -> impl Iterator<Item = (&'a Player, &'a UserInfo, SteamID)> {
    players
        .filter(|p| p.in_pvs && p.state == PlayerState::Alive)
        .filter_map(|p| p.info.as_ref().map(|i| (p, i)))
        .filter_map(|(p, i)| SteamID::from_steam3(&i.steam_id).ok().map(|id| (p, i, id)))
}

pub fn player_with_sid<'a>(players: impl Iterator<Item = &'a Player>, steam_id: SteamID) -> Option<(&'a Player, &'a UserInfo)> {
    players
        .filter(|p| p.in_pvs && p.state == PlayerState::Alive)
        .filter_map(|p| p.info.as_ref().and_then(|i| SteamID::from_steam3(&i.steam_id).ok().map(|id| (p, i, id))))
        .find(|(_, _, id)| *id == steam_id)
        .map(|(p, i, _)| (p, i))
}

pub fn player_with_uid<'a>(players: impl Iterator<Item = &'a Player>, user_id: UserId) -> Option<(&'a Player, &'a UserInfo, SteamID)> {
    for (player, info, sid) in valid_players(players) {
        if info.user_id == user_id {
            return Some((player, info, sid));
        }
    }
    None
}
