// Put individual functions with widespread utility in here.
// For more complicated structures, consider making your own file in the /util directory.

use crate::base::cheat_analyser_base::{Player, PlayerState};
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use steamid_ng::SteamID;
use tf_demo_parser::demo::message::packetentities::EntityId;

// Compute the difference in viewangles. We have to account for the fact viewangles are in a circle.
// E.g. If viewangle goes from 350 to 10 degrees, we want to return 20 degrees.
pub fn viewangle_delta(current_angle: ViewAngle, previous_angle: ViewAngle, tick_delta: u32) -> (f32, f32) {
    let tick_delta = if tick_delta < 1 { 1 } else { tick_delta };
    let va_delta = {
        let diff = (current_angle.yaw - previous_angle.yaw).rem_euclid(360.0);
        if diff > 180.0 { diff - 360.0 } else { diff }
    } / tick_delta as f32;
    let pa_delta = (current_angle.pitch - previous_angle.pitch) / tick_delta as f32;
    (va_delta, pa_delta)
}

// Since TF2 has an object limit of 2048, the lowest 11 bits of the handle ID represent the entity ID.
// Source: https://developer.valvesoftware.com/wiki/CHandle
#[allow(dead_code)]
pub fn handle_to_entid(handle: u32) -> EntityId {
    let entid = handle & 0x7FF;
    EntityId::from(entid)
}

#[derive(Debug, Clone, Copy)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ViewAngle {
    /// yaw aka `Player.view_angle`
    pub yaw: f32,
    /// pitch aka `Player.pitch_angle`
    pub pitch: f32,
}

impl ViewAngle {
    /// calculate the normalized `Vec3` that points in `ViewAngle`
    pub fn to_unit_vec(self) -> Vec3 {
        let Self { yaw, pitch } = self;

        let yaw = yaw.to_radians();
        let pitch = pitch.to_radians();
        Vec3 {
            x: pitch.cos() * yaw.cos(),
            y: pitch.sin() * yaw.sin(),
            z: -pitch.sin(),
        }
    }

    /// Compute the total angular difference between two angles in degrees
    pub fn delta(self, other: Self) -> f32 {
        let vec1 = self.to_unit_vec();
        let vec2 = other.to_unit_vec();

        let dot = vec1.dot(vec2);
        dot.acos().to_degrees()
    }
}

impl Vec3 {
    pub fn dot(self, rhs: Self) -> f32 {
        self.x * rhs.x + self.y * rhs.y + self.z * rhs.z
    }
}

impl From<&Player> for ViewAngle {
    fn from(value: &Player) -> Self {
        Self {
            yaw: value.view_angle,
            pitch: value.pitch_angle,
        }
    }
}

impl From<tf_demo_parser::demo::vector::Vector> for Vec3 {
    fn from(value: tf_demo_parser::demo::vector::Vector) -> Self {
        let tf_demo_parser::demo::vector::Vector { x, y, z } = value;
        Self { x, y, z }
    }
}

pub fn valid_players<'a>(
    players: impl Iterator<Item = &'a Player>,
) -> impl Iterator<Item = (&'a Player, &'a tf_demo_parser::demo::parser::analyser::UserInfo, SteamID)> {
    players
        .filter(|p| p.in_pvs && p.state == PlayerState::Alive)
        .filter_map(|p| p.info.as_ref().map(|i| (p, i)))
        .filter_map(|(p, i)| SteamID::from_steam3(&i.steam_id).ok().map(|id| (p, i, id)))
}

pub fn valid_player<'a>(
    players: impl Iterator<Item = &'a Player>,
    steam_id: SteamID,
) -> Option<(&'a Player, &'a tf_demo_parser::demo::parser::analyser::UserInfo)> {
    players
        .filter(|p| p.in_pvs && p.state == PlayerState::Alive)
        .filter_map(|p| p.info.as_ref().and_then(|i| SteamID::from_steam3(&i.steam_id).ok().map(|id| (p, i, id))))
        .find(|(_, _, id)| *id == steam_id)
        .map(|(p, i, _)| (p, i))
}

/*
 NOTE:
 This struct only exists because of the assumption that it is faster to use `Vec`s as `HashMap`s.
 That should generally be the case for fewer values, since hashing has some overhead.
 Some quick lookups online show that ~100 elements `HashMap`s can become viable which matches
 TF2's player limit. Since most games are played on 24 or 32 player servers, storing player related
 data in a HashMap means there are at most 24 or 32 entries, making `HashMap`s slower on average.
 Just so cache locality is better, the keys all live in their own `Vec`.
 If this turns out to be slower or not worth it for some reason, feel free to revert this change.
*/
pub struct KeyValueVec<K, V> {
    key: Vec<K>,
    value: Vec<V>,
}

impl<K: Clone, V: Clone> Clone for KeyValueVec<K, V> {
    fn clone(&self) -> Self {
        Self {
            key: self.key.clone(),
            value: self.value.clone(),
        }
    }
}

impl<'a, K, V> KeyValueVec<K, V>
where
    K: 'a,
    K: std::cmp::PartialEq<K>,
{
    #[must_use]
    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        let index = self.get_index(&key);
        match index {
            Some(index) => {
                let present_value = unsafe { self.value.get_unchecked_mut(index) };
                Some(std::mem::replace(present_value, value))
            }
            None => {
                self.key.push(key);
                self.value.push(value);
                None
            }
        }
    }

    pub fn get(&'a self, key: &'a K) -> Option<&'a V> {
        let index = self.get_index(key)?;
        Some(unsafe { self.value.get_unchecked(index) })
    }

    pub fn get_mut(&'a mut self, key: &'a K) -> Option<&'a mut V> {
        let index = self.get_index(key)?;
        Some(unsafe { self.value.get_unchecked_mut(index) })
    }

    pub fn remove(&'a mut self, key: &'a K) -> Option<V> {
        let index = self.get_index(key)?;
        self.key.swap_remove(index);
        Some(self.value.swap_remove(index))
    }

    fn get_index(&'a self, key: &'a K) -> Option<usize> {
        self.key.iter().enumerate().find(|(_, k)| **k == *key).map(move |(i, _)| i)
    }
}

impl<'a, K, V> KeyValueVec<K, V>
where
    K: 'a,
    V: 'a,
{
    /// creates a new empty `KeyValueVec<K, V>` without allocating space
    pub fn new() -> Self {
        Self {
            key: Vec::new(),
            value: Vec::new(),
        }
    }

    /// iterates over the key-value pairs
    pub fn iter(&'a self) -> std::iter::Zip<std::slice::Iter<'a, K>, std::slice::Iter<'a, V>> {
        self.key.iter().zip(self.value.iter())
    }
}

impl<K, V> Default for KeyValueVec<K, V> {
    fn default() -> Self {
        Self {
            key: Vec::new(),
            value: Vec::new(),
        }
    }
}

impl<K: Debug, V: Debug> Debug for KeyValueVec<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{{{}}}", self.iter().map(|(k, v)| format!("{:?}: {:?}", k, v)).join(", "))
    }
}
