use crate::base::cheat_analyser_base::{Player, PlayerState};
use steamid_ng::SteamID;
use tf_demo_parser::demo::parser::analyser::{UserId, UserInfo};

/// get an iterator over players with steam id's and `Some(UserInfo)`
pub fn valid_players<'a>(players: impl Iterator<Item = &'a Player>) -> impl Iterator<Item = (&'a Player, &'a UserInfo, SteamID)> {
    players
        // .filter(|p| p.in_pvs && p.state == PlayerState::Alive)
        .filter_map(|p| p.info.as_ref().map(|i| (p, i)))
        .filter_map(|(p, i)| SteamID::from_steam3(&i.steam_id).ok().map(|id| (p, i, id)))
}

/// filters out players that are not alive or not in pvs
pub fn players_alive_and_in_pvs<'a>(players: impl Iterator<Item = &'a Player>) -> impl Iterator<Item = &'a Player> {
    players.filter(|p| p.in_pvs && matches!(p.state, PlayerState::Alive))
}

/// finds a player with the given SteamID
///
/// the player is guaranteed to have a steamid and `Some(UserInfo)`.
/// There player may be dead or not in pvs. If this is not desired,
/// filter out the players using `players_alive_and_in_pvs`.
pub fn player_with_sid<'a>(players: impl Iterator<Item = &'a Player>, steam_id: SteamID) -> Option<(&'a Player, &'a UserInfo)> {
    players
        .filter_map(|p| p.info.as_ref().and_then(|i| SteamID::from_steam3(&i.steam_id).ok().map(|id| (p, i, id))))
        .find(|(_, _, id)| *id == steam_id)
        .map(|(p, i, _)| (p, i))
}

/// finds a player with the given UserId
///
/// the player is guaranteed to have a steamid and `Some(UserInfo)`.
/// There player may be dead or not in pvs. If this is not desired,
/// filter out the players using `players_alive_and_in_pvs`.
pub fn player_with_uid<'a>(players: impl Iterator<Item = &'a Player>, user_id: UserId) -> Option<(&'a Player, &'a UserInfo, SteamID)> {
    valid_players(players).find(|(_, info, _)| info.user_id == user_id)
}
