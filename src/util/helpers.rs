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


use tf_demo_parser::demo::parser::analyser::Class;

pub fn weapon_name_from_id_or_class(item_def: Option<u16>, class_name: &str, player_class: Class) -> String {
    if let Some(id) = item_def {
        let name = match id {
            0 | 190 | 221 => match player_class {
                Class::Scout => "Bat",
                Class::Soldier => "Shovel",
                Class::Pyro => "Fire Axe",
                Class::Demoman => "Bottle",
                Class::Heavy => "Fists",
                Class::Engineer => "Wrench",
                Class::Medic => "Bonesaw",
                Class::Sniper => "Kukri",
                Class::Spy => "Knife",
                _ => "Melee",
            },
            13 | 200 => "Scattergun",
            23 | 209 => "Pistol",
            44 => "Sandman",
            45 => "Force-A-Nature",
            46 => "Bonk! Atomic Punch",
            163 => "Crit-a-Cola",
            220 => "Shortstop",
            222 => "Mad Milk",
            317 => "Candy Cane",
            325 => "Boston Basher",
            349 => "Sun-on-a-Stick",
            355 => "Fan O'War",
            448 => "Soda Popper",
            449 => "Atomizer",
            450 => "Winger",
            772 => "Baby Face's Blaster",
            773 => "Pretty Boy's Pocket Pistol",
            812 => "Flying Guillotine",
            1103 => "Back Scatter",
            1121 => "Mutated Milk",

            18 | 205 => "Rocket Launcher",
            10 | 199 => "Shotgun",
            6 | 196 => "Shovel",
            127 => "Direct Hit",
            128 => "Equalizer",
            129 => "Buff Banner",
            133 => "Gunboats",
            228 => "Black Box",
            237 => "Rocket Jumper",
            265 => "Battalion's Backup",
            354 => "Concheror",
            414 => "Liberty Launcher",
            415 => "Reserve Shooter",
            416 => "Market Gardener",
            441 => "Cow Mangler 5000",
            442 => "Righteous Bison",
            444 => "Mantreads",
            447 => "Disciplinary Action",
            730 => "Beggar's Bazooka",
            775 => "Escape Plan",
            1104 => "Air Strike",
            1101 => "B.A.S.E. Jumper",

            21 | 208 => "Flame Thrower",
            2 | 192 => "Fire Axe",
            38 => "Axtinguisher",
            39 => "Flare Gun",
            40 => "Backburner",
            118 => "Homewrecker",
            214 => "Powerjack",
            215 => "Degreaser",
            326 => "Back Scratcher",
            348 => "Sharpened Volcano Fragment",
            351 => "Detonator",
            400 => "Postal Pummeler",
            593 => "Third Degree",
            595 => "Manmelter",
            740 => "Scorch Shot",
            810 => "Rainblower",
            813 => "Lollichop",
            1178 => "Dragon's Fury",
            1179 => "Thermal Thruster",
            1180 => "Gas Passer",
            1181 => "Hot Hand",

            19 | 206 => "Grenade Launcher",
            20 | 207 => "Stickybomb Launcher",
            1 | 191 => "Bottle",
            130 => "Scottish Resistance",
            131 => "Chargin' Targe",
            132 => "Eyelander",
            172 => "Scotsman's Skullcutter",
            266 => "Horseless Headless Horsemann's Headtaker",
            307 => "Ullapool Caber",
            308 => "Loch-n-Load",
            327 => "Claidheamh Mòr",
            405 => "Ali Baba's Wee Booties",
            406 => "Splendid Screen",
            482 => "Nessie's Nine Iron",
            608 => "Bootlegger",
            996 => "Loose Cannon",
            1099 => "Tide Turner",
            1150 => "Quickiebomb Launcher",
            1151 => "Iron Bomber",

            15 | 202 => "Minigun",
            11 => "Shotgun",
            5 | 195 => "Fists",
            42 => "Sandvich",
            43 => "Killing Gloves of Boxing",
            159 => "Dalokohs Bar",
            239 => "Gloves of Running Urgently",
            310 => "Warrior's Spirit",
            311 => "Buffalo Steak Sandvich",
            312 => "Brass Beast",
            424 => "Tomislav",
            425 => "Family Business",
            426 => "Eviction Notice",
            656 => "Holiday Punch",
            811 => "Huo-Long Heater",
            850 => "Deflector",
            1190 => "Second Banana",

            9 => "Shotgun",
            22 => "Pistol",
            7 | 197 => "Wrench",
            140 => "Wrangler",
            141 => "Frontier Justice",
            142 => "Gunslinger",
            155 => "Southern Hospitality",
            169 => "Golden Wrench",
            329 => "Jag",
            527 => "Widowmaker",
            528 => "Short Circuit",
            588 => "Pomson 6000",
            589 => "Eureka Effect",
            1055 => "Rescue Ranger",
            1086 => "Giger Counter",

            17 | 204 => "Syringe Gun",
            29 | 211 => "Medi Gun",
            8 | 198 => "Bonesaw",
            35 => "Kritzkrieg",
            36 => "Blutsauger",
            37 => "Ubersaw",
            173 => "Vita-Saw",
            305 => "Crusader's Crossbow",
            411 => "Quick-Fix",
            412 => "Overdose",
            998 => "Vaccinator",

            14 | 201 => "Sniper Rifle",
            16 | 203 => "SMG",
            3 | 193 => "Kukri",
            56 => "Huntsman",
            57 => "Razorback",
            58 => "Jarate",
            171 => "Tribalman's Shiv",
            230 => "Sydney Sleeper",
            231 => "Darwin's Danger Shield",
            232 => "Bushwacka",
            401 => "Shahanshah",
            402 => "Bazaar Bargain",
            526 => "Machina",
            751 => "Cleaner's Carbine",
            752 => "Hitman's Heatmaker",
            851 => "AWPer Hand",
            1092 => "Fortified Compound",
            1098 => "Classic",
            1105 => "Self-Aware Beauty Mark",

            24 | 210 => "Revolver",
            735 | 736 => "Sapper",
            4 | 194 => "Knife",
            27 => "Disguise Kit",
            30 => "Invis Watch",
            59 => "Dead Ringer",
            60 => "Cloak and Dagger",
            61 => "Ambassador",
            161 => "Big Kill",
            224 => "L'Etranger",
            225 => "Your Eternal Reward",
            460 => "Enforcer",
            461 => "Big Earner",
            525 => "Diamondback",
            572 => "Unarmed Combat",
            638 => "Sharp Dresser",
            649 => "Spy-cicle",
            830 => "Red-Tape Recorder",
            264 => "Frying Pan",
            423 => "Saxxy",
            474 => "Conscientious Objector",
            880 => "Freedom Staff",
            939 => "Bat Outta Hell",
            1013 => "Ham Shank",
            1127 => "Crossing Guard",
            1123 => "Necro Smasher",
            1182 => "Prinny Machete",

            _ => "",
        };
        if !name.is_empty() {
            return name.to_string();
        }
    }

    let clean = class_name
        .strip_prefix("CTF")
        .or_else(|| class_name.strip_prefix("C"))
        .unwrap_or(class_name);
    let clean = clean.strip_prefix("Weapon").unwrap_or(clean);

    match clean {
        "ScatterGun" | "Scattergun" => "Scattergun".to_string(),
        "Minigun" => "Minigun".to_string(),
        "SniperRifle" => "Sniper Rifle".to_string(),
        "SniperRifleDecap" => "Bazaar Bargain".to_string(),
        "CompoundBow" => "Huntsman".to_string(),
        "RocketLauncher_AirStrike" | "RocketLauncher_Airstrike" => "Air Strike".to_string(),
        "RocketLauncher_DirectHit" => "Direct Hit".to_string(),
        "RocketLauncher" => "Rocket Launcher".to_string(),
        "PipebombLauncher" => "Stickybomb Launcher".to_string(),
        "GrenadeLauncher" => "Grenade Launcher".to_string(),
        "Cannon" => "Loose Cannon".to_string(),
        "Knife" => "Knife".to_string(),
        "Revolver" => "Revolver".to_string(),
        "Medigun" => "Medi Gun".to_string(),
        "FlameThrower" => "Flame Thrower".to_string(),
        "Pistol" | "Pistol_Scout" | "Pistol_ScoutPrimary" | "Pistol_ScoutSecondary" => "Pistol".to_string(),
        "Shotgun" | "Shotgun_Soldier" | "Shotgun_Pyro" | "Shotgun_HWG" | "Shotgun_Engineer" | "Shotgun_Primary" => "Shotgun".to_string(),
        "Wrench" => "Wrench".to_string(),
        "Bonesaw" => "Bonesaw".to_string(),
        "Club" => "Kukri".to_string(),
        "Sword" => "Eyelander".to_string(),
        "Shovel" => "Shovel".to_string(),
        "FireAxe" => "Fire Axe".to_string(),
        "Bat" | "Bat_Wood" | "Bat_Fish" | "Bat_Giftwrap" => "Bat".to_string(),
        "Fists" => "Fists".to_string(),
        "SyringeGun" => "Syringe Gun".to_string(),
        "Crossbow" => "Crusader's Crossbow".to_string(),
        "SMG" => "SMG".to_string(),
        "Jar" | "JarMilk" | "JarGas" => "Jarate".to_string(),
        "BuffItem" => "Buff Banner".to_string(),
        "LunchBox" | "LunchBox_Drink" => "Sandvich".to_string(),
        "LaserPointer" => "Wrangler".to_string(),
        "Sapper" => "Sapper".to_string(),
        "Invis" => "Invis Watch".to_string(),
        "" => "unknown".to_string(),
        other => other.to_string(), }}