// Put individual functions with widespread utility in here.
// For more complicated structures, consider making your own file in the /util directory.

pub mod geometry;
pub mod key_value_vec;
pub mod player_finders;

pub use geometry::{Vec3, ViewAngle};
pub use key_value_vec::KeyValueVec;

use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hasher};
use std::path::{Path, PathBuf};
use tf_demo_parser::demo::message::packetentities::EntityId;

/// Fast non-cryptographic hasher (the FxHash used by rustc). Much cheaper than the default
/// SipHash for maps keyed by SteamIDs, entity ids and similar values looked up every tick.
#[derive(Default, Clone, Copy)]
pub struct FxHasher {
    hash: u64,
}

impl FxHasher {
    #[inline]
    fn add(&mut self, value: u64) {
        self.hash = (self.hash.rotate_left(5) ^ value).wrapping_mul(0x51_7c_c1_b7_27_22_0a_95);
    }
}

impl Hasher for FxHasher {
    #[inline]
    fn finish(&self) -> u64 {
        self.hash
    }

    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        for chunk in bytes.chunks(8) {
            let mut word = [0u8; 8];
            word[..chunk.len()].copy_from_slice(chunk);
            self.add(u64::from_le_bytes(word));
        }
    }

    #[inline]
    fn write_u8(&mut self, value: u8) {
        self.add(u64::from(value));
    }

    #[inline]
    fn write_u16(&mut self, value: u16) {
        self.add(u64::from(value));
    }

    #[inline]
    fn write_u32(&mut self, value: u32) {
        self.add(u64::from(value));
    }

    #[inline]
    fn write_u64(&mut self, value: u64) {
        self.add(value);
    }

    #[inline]
    fn write_usize(&mut self, value: usize) {
        self.add(value as u64);
    }
}

pub type FxBuildHasher = BuildHasherDefault<FxHasher>;
pub type FxHashMap<K, V> = HashMap<K, V, FxBuildHasher>;

// Since TF2 has an object limit of 2048, the lowest 11 bits of the handle ID represent the entity ID.
// Source: https://developer.valvesoftware.com/wiki/CHandle
#[allow(dead_code)]
pub fn handle_to_entid(handle: u32) -> EntityId {
    let entid = handle & 0x7FF;
    EntityId::from(entid)
}

/*
// TODO: check what this does
fn angles_to_unit_vec(yaw: f32, pitch: f32) -> (f32, f32, f32) {
    let yaw = yaw.to_radians();
    let pitch = pitch.to_radians();
    (pitch.cos() * yaw.sin(), pitch.sin(), pitch.cos() * yaw.cos())
}
// */

/// Mirrors CGlobalVarsBase::GetNetworkBase + RecvProxy_SimulationTime. `Player::simtime` holds the
/// raw 8-bit network value relative to a per-entity base that advances in 100-tick steps, so it
/// needs the server tick (NetTick) of the snapshot it came from to decode the full simulation tick.
pub fn reconstruct_simtick(server_tick: i64, entindex: i64, value: i64) -> i64 {
    let base = 100 * ((server_tick - entindex % 32) / 100);
    let mut t = base + value;
    while t < server_tick - 127 {
        t += 256;
    }
    while t > server_tick + 127 {
        t -= 256;
    }
    t
}

use tf_demo_parser::demo::parser::analyser::Class;

pub fn weapon_name_from_id_or_class(
    item_def: Option<u16>,
    class_name: &str,
    player_class: Class,
) -> &str {
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
            return name;
        }
    }

    let clean = class_name
        .strip_prefix("CTF")
        .or_else(|| class_name.strip_prefix("C"))
        .unwrap_or(class_name);
    let clean = clean.strip_prefix("Weapon").unwrap_or(clean);

    match clean {
        "ScatterGun" | "Scattergun" => "Scattergun",
        "Minigun" => "Minigun",
        "SniperRifle" => "Sniper Rifle",
        "SniperRifleDecap" => "Bazaar Bargain",
        "CompoundBow" => "Huntsman",
        "RocketLauncher_AirStrike" | "RocketLauncher_Airstrike" => "Air Strike",
        "RocketLauncher_DirectHit" => "Direct Hit",
        "RocketLauncher" => "Rocket Launcher",
        "PipebombLauncher" => "Stickybomb Launcher",
        "GrenadeLauncher" => "Grenade Launcher",
        "Cannon" => "Loose Cannon",
        "Knife" => "Knife",
        "Revolver" => "Revolver",
        "Medigun" => "Medi Gun",
        "FlameThrower" => "Flame Thrower",
        "Pistol" | "Pistol_Scout" | "Pistol_ScoutPrimary" | "Pistol_ScoutSecondary" => "Pistol",
        "Shotgun" | "Shotgun_Soldier" | "Shotgun_Pyro" | "Shotgun_HWG" | "Shotgun_Engineer"
        | "Shotgun_Primary" => "Shotgun",
        "Wrench" => "Wrench",
        "Bonesaw" => "Bonesaw",
        "Club" => "Kukri",
        "Sword" => "Eyelander",
        "Shovel" => "Shovel",
        "FireAxe" => "Fire Axe",
        "Bat" | "Bat_Wood" | "Bat_Fish" | "Bat_Giftwrap" => "Bat",
        "Fists" => "Fists",
        "SyringeGun" => "Syringe Gun",
        "Crossbow" => "Crusader's Crossbow",
        "SMG" => "SMG",
        "Jar" | "JarMilk" | "JarGas" => "Jarate",
        "BuffItem" => "Buff Banner",
        "LunchBox" | "LunchBox_Drink" => "Sandvich",
        "LaserPointer" => "Wrangler",
        "Sapper" => "Sapper",
        "Invis" => "Invis Watch",
        "" => "unknown",
        other => other,
    }
}

#[cfg(windows)]
pub fn get_steam_install_path() -> Option<std::path::PathBuf> {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;

    #[link(name = "advapi32")]
    extern "system" {
        fn RegOpenKeyExW(
            hKey: usize,
            lpSubKey: *const u16,
            ulOptions: u32,
            samDesired: u32,
            phkResult: *mut usize,
        ) -> i32;
        fn RegQueryValueExW(
            hKey: usize,
            lpValueName: *const u16,
            lpReserved: *mut u32,
            lpType: *mut u32,
            lpData: *mut u8,
            lpcbData: *mut u32,
        ) -> i32;
        fn RegCloseKey(hKey: usize) -> i32;
    }

    const HKEY_LOCAL_MACHINE: usize = 0x80000002;
    const KEY_READ: u32 = 0x20019;

    let subkeys = [
        "SOFTWARE\\WOW6432Node\\Valve\\Steam\0",
        "SOFTWARE\\Valve\\Steam\0",
    ];

    for subkey in subkeys {
        let wide_subkey: Vec<u16> = subkey.encode_utf16().collect();
        let mut hkey = 0usize;
        unsafe {
            if RegOpenKeyExW(
                HKEY_LOCAL_MACHINE,
                wide_subkey.as_ptr(),
                0,
                KEY_READ,
                &mut hkey,
            ) == 0
            {
                let val_name: Vec<u16> = "InstallPath\0".encode_utf16().collect();
                let mut buf_len = 0u32;
                if RegQueryValueExW(
                    hkey,
                    val_name.as_ptr(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    &mut buf_len,
                ) == 0
                    && buf_len > 0
                {
                    let mut data = vec![0u8; buf_len as usize];
                    if RegQueryValueExW(
                        hkey,
                        val_name.as_ptr(),
                        std::ptr::null_mut(),
                        std::ptr::null_mut(),
                        data.as_mut_ptr(),
                        &mut buf_len,
                    ) == 0
                    {
                        RegCloseKey(hkey);
                        let u16_slice: &[u16] =
                            std::slice::from_raw_parts(data.as_ptr() as *const u16, data.len() / 2);
                        let end = u16_slice
                            .iter()
                            .position(|&c| c == 0)
                            .unwrap_or(u16_slice.len());
                        let os_str = OsString::from_wide(&u16_slice[..end]);
                        return Some(std::path::PathBuf::from(os_str));
                    }
                }
                RegCloseKey(hkey);
            }
        }
    }
    None
}

#[cfg(not(windows))]
pub fn get_steam_install_path() -> Option<std::path::PathBuf> {
    std::env::var("HOME")
        .map(|home| {
            std::path::Path::new(&home)
                .join(".local")
                .join("share")
                .join("Steam")
        })
        .ok()
}

fn resolve_tf2_demos_dir(tf_dir: std::path::PathBuf) -> std::path::PathBuf {
    let demos_sub = tf_dir.join("demos");
    if demos_sub.is_dir() {
        demos_sub
    } else {
        tf_dir
    }
}

pub fn find_default_tf2_dir() -> Option<std::path::PathBuf> {
    // 1. Environment variable override
    if let Ok(dir) = std::env::var("TF2_DIR") {
        let p = std::path::PathBuf::from(dir);
        if p.is_dir() {
            if p.file_name()
                .is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case("demos"))
            {
                return Some(p);
            }
            if p.file_name()
                .is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case("tf"))
            {
                return Some(resolve_tf2_demos_dir(p));
            }
            let tf_sub = p.join("tf");
            if tf_sub.is_dir() {
                return Some(resolve_tf2_demos_dir(tf_sub));
            }
            return Some(resolve_tf2_demos_dir(p));
        }
    }

    // 2. Windows checks
    #[cfg(windows)]
    {
        // 2a. Common standard install locations
        let common = std::path::PathBuf::from(
            r"C:\Program Files (x86)\Steam\steamapps\common\Team Fortress 2\tf",
        );
        if common.is_dir() {
            return Some(resolve_tf2_demos_dir(common));
        }

        let common_pf =
            std::path::PathBuf::from(r"C:\Program Files\Steam\steamapps\common\Team Fortress 2\tf");
        if common_pf.is_dir() {
            return Some(resolve_tf2_demos_dir(common_pf));
        }

        // 2b. From Steam install path via registry
        if let Some(steam_path) = get_steam_install_path() {
            let tf2 = steam_path
                .join("steamapps")
                .join("common")
                .join("Team Fortress 2")
                .join("tf");
            if tf2.is_dir() {
                return Some(resolve_tf2_demos_dir(tf2));
            }

            // 2c. Check additional Steam library folders from libraryfolders.vdf
            let vdf_path = steam_path.join("steamapps").join("libraryfolders.vdf");
            if let Ok(vdf) = std::fs::read_to_string(&vdf_path) {
                for line in vdf.lines() {
                    let trimmed = line.trim();
                    if trimmed.starts_with("\"path\"") {
                        if let Some(first_quote) = trimmed.find('"') {
                            if let Some(second_quote) = trimmed[first_quote + 1..].find('"') {
                                let rem = &trimmed[first_quote + 1 + second_quote + 1..];
                                let path_val = rem.trim().trim_matches('"');
                                let lib_folder =
                                    std::path::PathBuf::from(path_val.replace(r"\\", r"\"));
                                let tf2_lib = lib_folder
                                    .join("steamapps")
                                    .join("common")
                                    .join("Team Fortress 2")
                                    .join("tf");
                                if tf2_lib.is_dir() {
                                    return Some(resolve_tf2_demos_dir(tf2_lib));
                                }
                            }
                        }
                    }
                }
            }
        }

        // 2d. Common secondary drive paths
        for drive in ["D", "E", "F", "G"] {
            for sub in [
                format!(
                    r"{}:\SteamLibrary\steamapps\common\Team Fortress 2\tf",
                    drive
                ),
                format!(r"{}:\Steam\steamapps\common\Team Fortress 2\tf", drive),
                format!(
                    r"{}:\Games\Steam\steamapps\common\Team Fortress 2\tf",
                    drive
                ),
            ] {
                let p = std::path::PathBuf::from(sub);
                if p.is_dir() {
                    return Some(resolve_tf2_demos_dir(p));
                }
            }
        }
    }

    // 3. Linux / Unix checks
    #[cfg(not(windows))]
    {
        if let Ok(home) = std::env::var("HOME") {
            let home_p = std::path::Path::new(&home);
            let candidates = [
                home_p.join(".local/share/Steam/steamapps/common/Team Fortress 2/tf"),
                home_p.join(".steam/steam/steamapps/common/Team Fortress 2/tf"),
                home_p.join(".steam/root/steamapps/common/Team Fortress 2/tf"),
                home_p
                    .join("Library/Application Support/Steam/steamapps/common/Team Fortress 2/tf"),
            ];
            for c in candidates {
                if c.is_dir() {
                    return Some(resolve_tf2_demos_dir(c));
                }
            }
        }
    }

    None
}

/// Formats a suspicious demo filename by appending the number of detections, e.g.:
/// `demo.dem` -> `demo(10 flags).dem`
/// If the filename already ends with a flags count tag (e.g. `(5 flags)` or `(1 flag)`),
/// it is replaced cleanly without chaining tags.
pub fn format_sus_demo_filename(original_filename: &str, flags_count: usize) -> String {
    let (stem, ext) = if let Some(idx) = original_filename.rfind('.') {
        (&original_filename[..idx], &original_filename[idx..])
    } else {
        (original_filename, ".dem")
    };

    let clean_stem = if let Some(open_paren) = stem.rfind('(') {
        if stem.ends_with(')') {
            let inner = &stem[open_paren + 1..stem.len() - 1];
            let inner_parts: Vec<&str> = inner.split_whitespace().collect();
            if inner_parts.len() == 2
                && inner_parts[0].chars().all(|c| c.is_ascii_digit())
                && (inner_parts[1].eq_ignore_ascii_case("flag")
                    || inner_parts[1].eq_ignore_ascii_case("flags"))
            {
                &stem[..open_paren]
            } else {
                stem
            }
        } else {
            stem
        }
    } else {
        stem
    };

    format!("{}({} flags){}", clean_stem, flags_count, ext)
}

/// Reads map name and duration directly from demo header.
pub fn read_demo_header_info(path: &Path) -> Result<(String, f32), String> {
    use bitbuffer::BitRead;
    use tf_demo_parser::Demo;
    use tf_demo_parser::demo::header::Header;

    let file_bytes = std::fs::read(path).map_err(|e| format!("Failed to read demo: {e}"))?;
    let demo = Demo::new(&file_bytes);
    let mut stream = demo.get_stream();
    let header =
        Header::read(&mut stream).map_err(|e| format!("Failed to read demo header: {e}"))?;
    Ok((header.map, header.duration))
}

/// Finds the TF2 replay storage directory (`tf/replay/client/replays`) relative to the demo or via Steam install paths.
// TODO: replace with steamlocate
pub fn find_tf2_replays_dir(demo_path: &Path) -> Option<PathBuf> {
    // 1. Check ancestors of demo_path
    for ancestor in demo_path.ancestors() {
        if ancestor
            .file_name()
            .and_then(|n| n.to_str())
            .map(|n| n.eq_ignore_ascii_case("tf"))
            .unwrap_or(false)
        {
            let replays = ancestor.join("replay").join("client").join("replays");
            return Some(replays);
        }
        let tf_sub = ancestor.join("tf");
        if tf_sub.is_dir() {
            let replays = tf_sub.join("replay").join("client").join("replays");
            return Some(replays);
        }
    }

    // 2. Check standard Steam library locations
    #[cfg(windows)]
    {
        let common = PathBuf::from(
            r"C:\Program Files (x86)\Steam\steamapps\common\Team Fortress 2\tf\replay\client\replays",
        );
        if common.is_dir() || common.parent().map_or(false, |p| p.is_dir()) {
            return Some(common);
        }

        if let Some(steam_path) = get_steam_install_path() {
            let tf2 = steam_path
                .join("steamapps")
                .join("common")
                .join("Team Fortress 2");
            if tf2.is_dir() {
                return Some(tf2.join("tf").join("replay").join("client").join("replays"));
            }
            let vdf_path = steam_path.join("steamapps").join("libraryfolders.vdf");
            if let Ok(vdf) = std::fs::read_to_string(&vdf_path) {
                for line in vdf.lines() {
                    let trimmed = line.trim();
                    if trimmed.starts_with("\"path\"") {
                        if let Some(first_quote) = trimmed.find('"') {
                            if let Some(second_quote) = trimmed[first_quote + 1..].find('"') {
                                let rem = &trimmed[first_quote + 1 + second_quote + 1..];
                                let path_val = rem.trim().trim_matches('"');
                                let lib_folder = PathBuf::from(path_val.replace(r"\\", r"\"));
                                let tf2_lib = lib_folder
                                    .join("steamapps")
                                    .join("common")
                                    .join("Team Fortress 2");
                                if tf2_lib.is_dir() {
                                    return Some(
                                        tf2_lib
                                            .join("tf")
                                            .join("replay")
                                            .join("client")
                                            .join("replays"),
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // 3. Fallback to default TF2 dir if known
    if let Some(demos_dir) = find_default_tf2_dir()
        && let Some(tf_dir) = demos_dir.parent()
    {
        let replays = tf_dir.join("replay").join("client").join("replays");
        return Some(replays);
    }

    None
}

/// Sanitizes early signon packets in a demo file by resetting non-zero ticks to 0.
///
/// In Source 1 engine demos, the initial packets prior to `dem_synctick` (command 3),
/// such as `dem_signon` (1) and `dem_datatables` (6), are stamped with the server host tick
/// at the time recording began (often tens of thousands of ticks in).
///
/// When the TF2 replay system skips to a take's starting tick (`CDemoPlayer::SkipToTick`),
/// the playback tick is computed relative to the first packet's tick:
/// `GetPlaybackTick() = host_tickcount - m_nStartTick`.
/// If the first packet tick is large (e.g. 21,984), `GetPlaybackTick()` immediately begins
/// at 21,984. Any take whose target tick is smaller than that value (e.g. tick 12,996) causes
/// `cmp target_tick, GetPlaybackTick()` to evaluate as already passed, cancelling the fast-forward
/// skip immediately at the start of the demo!
///
/// Zeroing ticks on all packets before `dem_synctick` (command 3) ensures that playback tick
/// starts at 0, allowing seeking to any tick in the demo without breaking demo parsing or checksums.
#[warn(clippy::unwrap_used)]
pub fn sanitize_demo_signon_ticks(demo_path: &Path) -> Result<usize, String> {
    const DEMO_HEADER_SIZE: usize = 1072;
    const DEM_HEADER_MAGIC: &[u8; 8] = b"HL2DEMO\0";

    let mut data = std::fs::read(demo_path)
        .map_err(|e| format!("Failed to read demo for signon tick sanitization: {e}"))?;

    if data.len() < DEMO_HEADER_SIZE {
        return Err("File is too small to be a valid Source demo".to_string());
    }
    if &data[..8] != DEM_HEADER_MAGIC {
        return Err("Not a valid HL2DEMO file (bad magic)".to_string());
    }

    let demo_protocol = i32::from_le_bytes(
        data[8..12]
            .try_into()
            .map_err(|_| "Failed to read demo protocol".to_string())?,
    );
    let has_slot = demo_protocol >= 4;

    let mut off = DEMO_HEADER_SIZE;
    let n = data.len();
    let mut modified_count = 0;

    while off < n {
        let [cmd] = read_exact(&data, off).map_err(|e| format!("could not read cmd: {e}"))?;
        off += 1;

        if cmd == 7 {
            // DEM_STOP
            break;
        }

        let tick_offset = off;
        let tick_val = i32::from_le_bytes(
            read_exact(&data, off).map_err(|e| format!("could not read tick_val: {e}"))?,
        );
        off += 4;

        if has_slot {
            if off + 1 > n {
                return Err(format!(
                    "Unexpected EOF reading player slot at offset {off}"
                ));
            }
            off += 1;
        }

        if cmd == 3 {
            // DEM_SYNCTICK reached: marks the end of the signon sequence.
            break;
        }

        // For all packets preceding DEM_SYNCTICK, zero out any non-zero tick
        if tick_val != 0 {
            data[tick_offset..tick_offset + 4].copy_from_slice(&0i32.to_le_bytes());
            modified_count += 1;
        }

        match cmd {
            1 | 2 => {
                // DEM_SIGNON / DEM_PACKET: democmdinfo (76) + in_seq (4) + out_seq (4) = 84 bytes
                off += 84;
                let length = i32::from_le_bytes(
                    read_exact(&data, off).map_err(|e| format!("could not read length: {e}"))?,
                );
                off += 4;
                if length < 0 || off + (length as usize) > n {
                    return Err(format!("Invalid packet length {length} at offset {off}"));
                }
                off += length as usize;
            }
            4 | 6 | 8 => {
                // DEM_CONSOLECMD / DEM_DATATABLES / DEM_STRINGTABLES
                let length = i32::from_le_bytes(
                    read_exact(&data, off).map_err(|e| format!("could not read length: {e}"))?,
                );
                off += 4;
                if length < 0 || off + (length as usize) > n {
                    return Err(format!(
                        "Invalid length {length} for cmd {cmd} at offset {off}"
                    ));
                }
                off += length as usize;
            }
            5 | 9 => {
                // DEM_USERCMD / DEM_CUSTOMDATA
                if off + 8 > n {
                    return Err(format!(
                        "Unexpected EOF reading header for cmd {cmd} at offset {off}"
                    ));
                }
                off += 4;
                let length = i32::from_le_bytes(
                    read_exact(&data, off).map_err(|e| format!("could not read length: {e}"))?,
                );
                off += 4;
                if length < 0 || off + (length as usize) > n {
                    return Err(format!(
                        "Invalid length {length} for cmd {cmd} at offset {off}"
                    ));
                }
                off += length as usize;
            }
            _ => {
                return Err(format!("Unknown demo command {cmd} at offset {off}"));
            }
        }
    }

    if modified_count > 0 {
        std::fs::write(demo_path, &data)
            .map_err(|e| format!("Failed to write sanitized demo file: {e}"))?;
    }

    Ok(modified_count)
}

/// Converts a demo file into a TF2 replay entry in `tf/replay/client/replays`.
/// Copies the demo file, sanitizes signon ticks to enable proper seeking,
/// updates `replays.dmx`, and writes `replay_{handle}.dmx`.
pub fn convert_demo_to_replay(
    demo_path: &Path,
    header_info: Option<(&str, f32)>,
) -> Result<u32, String> {
    let replays_dir = find_tf2_replays_dir(demo_path).ok_or_else(|| {
        "Could not locate TF2 replays folder (tf/replay/client/replays)".to_string()
    })?;

    std::fs::create_dir_all(&replays_dir)
        .map_err(|e| format!("Failed to create replays directory: {e}"))?;

    // 1. Ensure replays.dmx index exists
    let index_path = replays_dir.join("replays.dmx");
    if !index_path.exists() {
        std::fs::write(&index_path, "\"root\"\n{\n\t\"version\"\t\"0\"\n}\n")
            .map_err(|e| format!("Failed to create replays.dmx: {e}"))?;
    }

    let demo_filename = demo_path
        .file_name()
        .ok_or_else(|| "Invalid demo path".to_string())?
        .to_string_lossy()
        .to_string();

    // 2. Copy demo file into replays directory and sanitize signon ticks
    let target_demo_path = replays_dir.join(&demo_filename);
    if demo_path != target_demo_path && !target_demo_path.exists() {
        std::fs::copy(demo_path, &target_demo_path)
            .map_err(|e| format!("Failed to copy demo to replays folder: {e}"))?;
    }

    if target_demo_path.exists() {
        let _ = sanitize_demo_signon_ticks(&target_demo_path);
    }

    // 3. Extract map and duration
    let (map, duration) = if let Some((m, d)) = header_info {
        (m.to_string(), d)
    } else {
        read_demo_header_info(demo_path)?
    };

    // 4. Encode record time matching Valve's TF2 replay format
    let file_time = demo_path
        .metadata()
        .and_then(|m| m.created().or_else(|_| m.modified()))
        .unwrap_or_else(|_| std::time::SystemTime::now());

    let create_date: chrono::DateTime<chrono::Local> = chrono::DateTime::from(file_time);
    use chrono::{Datelike, Timelike};
    let kv_date = (create_date.day() - 1)
        | ((create_date.month() - 1) << 5)
        | ((create_date.year() as u32 - 2009) << 9);
    let kv_time = create_date.hour() | (create_date.minute() << 5) | (create_date.second() << 11);

    // 5. Generate unique replay handle in 0..i32::MAX
    let mut replay_handle: u32 = (std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u32)
        & 0x7FFF_FFFF;
    if replay_handle == 0 {
        replay_handle = 1;
    }
    while replays_dir
        .join(format!("replay_{replay_handle}.dmx"))
        .exists()
    {
        replay_handle =
            (replay_handle.wrapping_mul(1664525).wrapping_add(1013904223)) & 0x7FFF_FFFF;
        if replay_handle == 0 {
            replay_handle = 1;
        }
    }

    // 6. Write replay_{replay_handle}.dmx
    let generated = generate_replay_dmx_and_takes(
        replay_handle,
        &demo_filename,
        &map,
        duration,
        kv_date,
        kv_time,
        None,
        &[],
        None,
    );

    let dmx_path = replays_dir.join(format!("replay_{replay_handle}.dmx"));
    std::fs::write(&dmx_path, generated.main_dmx_content)
        .map_err(|e| format!("Failed to write replay DMX: {e}"))?;

    Ok(replay_handle)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplayTakeInput {
    pub tick: u32,
    pub player_name: String,
    pub algorithm: String,
    pub player_slot: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupedTake {
    pub tick_in: u32,
    pub tick_out: u32,
    pub first_tick: u32,
    pub last_tick: u32,
    pub player_name: String,
    pub player_slot: String,
    pub title: String,
}

#[warn(clippy::unwrap_used)]
pub fn group_takes_by_proximity(takes: &[ReplayTakeInput], gap_threshold: u32) -> Vec<GroupedTake> {
    if takes.is_empty() {
        return Vec::new();
    }

    let mut sorted_takes = takes.to_vec();
    sorted_takes.sort_by_key(|t| t.tick);

    let mut groups: Vec<Vec<ReplayTakeInput>> = Vec::new();
    let mut current_group: Vec<ReplayTakeInput> = Vec::new();
    let mut current_last_tick: u32 = 0;

    for take in sorted_takes {
        if !(current_group.is_empty()
            || (take.player_slot == current_group[0].player_slot
                && take.tick <= current_last_tick.saturating_add(gap_threshold)))
        {
            current_last_tick = take.tick;
            current_group.push(take);
        } else {
            groups.push(std::mem::take(&mut current_group));
            current_last_tick = take.tick;
            current_group.push(take);
        }
    }
    if !current_group.is_empty() {
        groups.push(current_group);
    }

    let mut result = Vec::new();
    for group in groups {
        let first_take = &group[0];
        let first_tick = first_take.tick;
        let last_tick = group
            .last()
            .expect("if indexing group[0] does not fail, then this won't either")
            .tick;
        let tick_in = first_tick.saturating_sub(300);
        let tick_out = last_tick.saturating_add(2000);

        let clean_name = first_take
            .player_name
            .replace('"', "")
            .replace(['\r', '\n', '\t'], " ");
        let clean_name = clean_name.trim();
        let clean_name = if clean_name.is_empty() {
            "unknown"
        } else {
            clean_name
        };

        let mut unique_algos: Vec<String> = Vec::new();
        for t in &group {
            let clean_algo = t
                .algorithm
                .replace('"', "")
                .replace(['\r', '\n', '\t'], " ");
            let clean_algo = clean_algo.trim().to_string();
            if !clean_algo.is_empty() && !unique_algos.contains(&clean_algo) {
                unique_algos.push(clean_algo);
            }
        }
        let algos_str = if unique_algos.is_empty() {
            "unknown".to_string()
        } else {
            unique_algos.join(", ")
        };

        let det_count = group.len();
        let title = if first_tick == last_tick {
            format!("{first_tick} - {algos_str} ({det_count})")
        } else {
            format!("{first_tick}-{last_tick} - {algos_str} ({det_count})")
        };

        result.push(GroupedTake {
            tick_in,
            tick_out,
            first_tick,
            last_tick,
            player_name: clean_name.to_string(),
            player_slot: first_take.player_slot.clone(),
            title,
        });
    }

    result
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedReplay {
    pub main_dmx_content: String,
    /// Vector of (filename, file_content) for each take
    pub takes: Vec<(String, String)>,
}

pub fn generate_replay_dmx_and_takes(
    replay_handle: u32,
    demo_filename: &str,
    map: &str,
    duration: f32,
    kv_date: u32,
    kv_time: u32,
    suspect_player_slot: Option<&str>,
    takes: &[ReplayTakeInput],
    custom_title: Option<&str>,
) -> GeneratedReplay {
    let player_slot_val = suspect_player_slot.unwrap_or("-1");
    let main_player_slot = suspect_player_slot
        .and_then(|s| s.parse::<i32>().ok())
        .filter(|&s| s >= 0)
        .map(|s| (s + 1).to_string())
        .unwrap_or_else(|| "-1".to_string());

    let mut edits_block = String::new();
    let mut generated_takes = Vec::new();

    let grouped_takes = group_takes_by_proximity(takes, 2000);

    if !grouped_takes.is_empty() {
        edits_block.push_str("\t\"edits\"\n\t{\n");
        for (i, take) in grouped_takes.iter().enumerate() {
            let edit_filename = format!("replay_{replay_handle}_edit_{i}.dmx");
            let tick_in = take.tick_in;
            let tick_out = take.tick_out;
            let title = &take.title;
            let target_slot = if !take.player_slot.is_empty() {
                &take.player_slot
            } else {
                player_slot_val
            };

            let ent_idx = target_slot
                .parse::<i32>()
                .ok()
                .filter(|&s| s >= 0)
                .map(|s| s + 1)
                .unwrap_or(1);

            let time_in_sec = (tick_in as f64) * 0.015151515;

            let take_content = format!(
                "\"Performance\"
{{
\t\"{time_in_sec:.6}\"
\t{{
\t\t\"type\"\t\t\"515\"
\t\t\"ent\"\t\t\"{ent_idx}\"
\t}}
\t\"{time_in_sec:.6}\"
\t{{
\t\t\"type\"\t\t\"516\"
\t\t\"cut\"\t\t\"1\"
\t\t\"pos\"\t\t\"0.000000 0.000000 0.000000\"
\t\t\"ang\"\t\t\"0.000000 0.000000 0.000000\"
\t\t\"fov\"\t\t\"90.000000\"
\t\t\"a\"\t\t\"4.999980\"
\t\t\"s\"\t\t\"2.999430\"
\t\t\"rf\"\t\t\"10.000002\"
\t}}
\t\"{time_in_sec:.6}\"
\t{{
\t\t\"type\"\t\t\"517\"
\t\t\"scale\"\t\t\"0.999989\"
\t}}
\t\"{time_in_sec:.6}\"
\t{{
\t\t\"type\"\t\t\"1\"
\t\t\"ent\"\t\t\"{ent_idx}\"
\t}}
}}
"
            );

            edits_block.push_str(&format!(
                "\t\t\"edit\"
\t\t{{
\t\t\t\"filename\"\t\t\"{edit_filename}\"
\t\t\t\"tick_in\"\t\t\"{tick_in}\"
\t\t\t\"tick_out\"\t\t\"{tick_out}\"
\t\t\t\"title\"\t\t\"{title}\"
\t\t}}
"
            ));

            generated_takes.push((edit_filename, take_content));
        }
        edits_block.push_str("\t}\n");
    }

    let title_str = custom_title
        .map(|s| s.replace('"', "").replace(['\r', '\n', '\t'], " "))
        .unwrap_or_else(|| demo_filename.to_string());

    let main_dmx_content = format!(
        "replay_{replay_handle}
{{
\t\"handle\"\t\"{replay_handle}\"
\t\"player_slot\"\t\"{main_player_slot}\"
\t\"map\"\t\"{map}\"
\t\"complete\"\t\"1\"
\t\"title\"\t\"{title_str}\"
\t\"recon_filename\"\t\"{demo_filename}\"
\t\"spawn_tick\"\t\"-1\"
\t\"death_tick\"\t\"-1\"
\t\"status\"\t\"3\"
\t\"length\"\t\"{duration}\"
\t\"record_time\"
\t{{
\t\t\"date\"\t\"{kv_date}\"
\t\t\"time\"\t\"{kv_time}\"
\t}}
{edits_block}}}
"
    );

    GeneratedReplay {
        main_dmx_content,
        takes: generated_takes,
    }
}

fn read_exact<const N: usize, T>(data: &[T], offset: usize) -> Result<[T; N], String>
where
    [T; N]: Default,
    T: Copy,
{
    let mut buffer: [T; N] = Default::default();
    for (i, field) in buffer.iter_mut().enumerate() {
        *field = *data.get(i + offset).ok_or(format!(
            "could not read byte at {}: early end of data stream",
            i + offset
        ))?;
    }
    Ok(buffer)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simtime_reconstruction_roundtrip() {
        for server_tick in [1000i64, 1099, 1100, 54321, 65535 + 300] {
            for ent in [1i64, 7, 31, 33] {
                for delta in [-24i64, -3, 0] {
                    let simtick = server_tick + delta;
                    let base = 100 * ((server_tick - ent % 32) / 100);
                    let value = (simtick - base) & 0xFF;
                    assert_eq!(reconstruct_simtick(server_tick, ent, value), simtick);
                }
            }
        }
    }

    #[test]
    fn test_default_tf2_demos_dir() {
        if let Some(dir) = find_default_tf2_dir() {
            println!("Resolved default TF2 demos dir: {}", dir.display());
            assert!(dir.is_dir());
            assert!(dir.ends_with("demos"));
        }
    }

    #[test]
    fn test_format_sus_demo_filename() {
        assert_eq!(
            format_sus_demo_filename("demo.dem", 10),
            "demo(10 flags).dem"
        );
        assert_eq!(
            format_sus_demo_filename("demo(10 flags).dem", 12),
            "demo(12 flags).dem"
        );
        assert_eq!(
            format_sus_demo_filename("demo(1 flag).dem", 5),
            "demo(5 flags).dem"
        );
        assert_eq!(
            format_sus_demo_filename("match.cp_badlands.dem", 3),
            "match.cp_badlands(3 flags).dem"
        );
        assert_eq!(
            format_sus_demo_filename("standalone", 1),
            "standalone(1 flags).dem"
        );
        assert_eq!(
            format_sus_demo_filename("demo(scout).dem", 2),
            "demo(scout)(2 flags).dem"
        );
    }

    #[test]
    fn test_generate_replay_dmx_no_takes() {
        let generated = generate_replay_dmx_and_takes(
            12345,
            "test.dem",
            "cp_badlands",
            120.0,
            1,
            2,
            None,
            &[],
            None,
        );
        assert!(
            generated
                .main_dmx_content
                .contains("\"player_slot\"\t\"-1\"")
        );
        assert!(!generated.main_dmx_content.contains("\"edits\""));
        assert!(generated.takes.is_empty());
    }

    #[test]
    fn test_generate_replay_dmx_single_suspect_takes() {
        let takes = vec![
            ReplayTakeInput {
                tick: 3500,
                player_name: "SuspectOne".to_string(),
                algorithm: "aimbot".to_string(),
                player_slot: "2".to_string(),
            },
            ReplayTakeInput {
                tick: 100,
                player_name: "SuspectOne\"".to_string(),
                algorithm: "bhop\r\n".to_string(),
                player_slot: "2".to_string(),
            },
        ];

        let generated = generate_replay_dmx_and_takes(
            99999,
            "match.dem",
            "pl_upward",
            300.5,
            10,
            20,
            Some("2"),
            &takes,
            Some("SuspectOne (match.dem)"),
        );

        // Check main DMX
        assert!(
            generated
                .main_dmx_content
                .contains("\"player_slot\"\t\"3\"")
        );
        assert!(
            generated
                .main_dmx_content
                .contains("\"title\"\t\"SuspectOne (match.dem)\"")
        );
        assert!(generated.main_dmx_content.contains("\"edits\""));
        assert!(
            generated
                .main_dmx_content
                .contains("\"filename\"\t\t\"replay_99999_edit_0.dmx\"")
        );
        assert!(
            generated
                .main_dmx_content
                .contains("\"filename\"\t\t\"replay_99999_edit_1.dmx\"")
        );
        assert_eq!(generated.takes.len(), 2);

        // Check take 0 (first sorted is tick 100: tick_in = 0, time_in_sec = 0.000000, tick_out = 2100)
        let (name0, content0) = &generated.takes[0];
        assert_eq!(name0, "replay_99999_edit_0.dmx");
        assert!(content0.contains("\"Performance\""));
        assert!(content0.contains("\"0.000000\""));
        assert!(content0.contains("\"type\"\t\t\"515\""));
        assert!(content0.contains("\"type\"\t\t\"516\""));
        assert!(content0.contains("\"cut\"\t\t\"1\""));
        assert!(content0.contains("\"type\"\t\t\"517\""));
        assert!(content0.contains("\"type\"\t\t\"1\""));
        assert!(content0.contains("\"ent\"\t\t\"3\""));

        // Check take 1 (tick 3500: tick_in = 3200, time_in_sec = 48.484848, tick_out = 5500)
        let (name1, content1) = &generated.takes[1];
        assert_eq!(name1, "replay_99999_edit_1.dmx");
        assert!(content1.contains("\"Performance\""));
        assert!(content1.contains("\"48.484848\""));
        assert!(content1.contains("\"type\"\t\t\"515\""));
        assert!(content1.contains("\"type\"\t\t\"516\""));
        assert!(content1.contains("\"cut\"\t\t\"1\""));
        assert!(content1.contains("\"type\"\t\t\"517\""));
        assert!(content1.contains("\"type\"\t\t\"1\""));
        assert!(content1.contains("\"ent\"\t\t\"3\""));
        // Quotes and newlines should be sanitized
        assert!(!generated.main_dmx_content.contains("SuspectOne\""));
    }

    #[test]
    fn test_group_takes_by_proximity() {
        let takes = vec![
            ReplayTakeInput {
                tick: 1000,
                player_name: "Cheater".to_string(),
                algorithm: "aimbot".to_string(),
                player_slot: "1".to_string(),
            },
            ReplayTakeInput {
                tick: 1500,
                player_name: "Cheater".to_string(),
                algorithm: "aimbot".to_string(),
                player_slot: "1".to_string(),
            },
            ReplayTakeInput {
                tick: 3200,
                player_name: "Cheater".to_string(),
                algorithm: "bhop".to_string(),
                player_slot: "1".to_string(),
            },
            ReplayTakeInput {
                tick: 7000,
                player_name: "Cheater".to_string(),
                algorithm: "warp".to_string(),
                player_slot: "1".to_string(),
            },
        ];

        let grouped = group_takes_by_proximity(&takes, 2000);
        assert_eq!(grouped.len(), 2);

        // Group 0: 1000 -> 1500 -> 3200
        assert_eq!(grouped[0].first_tick, 1000);
        assert_eq!(grouped[0].last_tick, 3200);
        assert_eq!(grouped[0].tick_in, 700); // 1000 - 300
        assert_eq!(grouped[0].tick_out, 5200); // 3200 + 2000
        assert_eq!(grouped[0].title, "1000-3200 - aimbot, bhop (3)");

        // Group 1: 7000
        assert_eq!(grouped[1].first_tick, 7000);
        assert_eq!(grouped[1].last_tick, 7000);
        assert_eq!(grouped[1].tick_in, 6700); // 7000 - 300
        assert_eq!(grouped[1].tick_out, 9000); // 7000 + 2000
        assert_eq!(grouped[1].title, "7000 - warp (1)");
    }

    #[test]
    fn test_generate_replay_dmx_multiple_suspects() {
        let takes = vec![
            ReplayTakeInput {
                tick: 1000,
                player_name: "PlayerA".to_string(),
                algorithm: "aimbot".to_string(),
                player_slot: "3".to_string(),
            },
            ReplayTakeInput {
                tick: 2000,
                player_name: "PlayerB".to_string(),
                algorithm: "warp".to_string(),
                player_slot: "7".to_string(),
            },
        ];

        let generated = generate_replay_dmx_and_takes(
            55555,
            "multi.dem",
            "cp_granary",
            500.0,
            1,
            1,
            Some("-1"), // multiple suspects keep main player_slot as -1
            &takes,
            None,
        );

        assert!(
            generated
                .main_dmx_content
                .contains("\"player_slot\"\t\"-1\"")
        );
        assert_eq!(generated.takes.len(), 2);

        // Take 0 target slot is 3 -> ent 4
        let (_, content0) = &generated.takes[0];
        assert!(content0.contains("\"ent\"\t\t\"4\""));

        // Take 1 target slot is 7 -> ent 8
        let (_, content1) = &generated.takes[1];
        assert!(content1.contains("\"ent\"\t\t\"8\""));
    }

    #[test]
    fn test_sanitize_demo_signon_ticks() {
        // Construct a mock demo with 1072 byte header + 1 signon packet (cmd 1, tick 21984) + synctick (cmd 3, tick 0) + stop (cmd 7)
        let mut mock_data = vec![0u8; 1072];
        mock_data[0..8].copy_from_slice(b"HL2DEMO\0");
        mock_data[8..12].copy_from_slice(&4i32.to_le_bytes()); // protocol 4

        // Frame 0: cmd 1 (signon), tick 21984, slot 0
        mock_data.push(1); // cmd
        mock_data.extend_from_slice(&21984i32.to_le_bytes()); // tick
        mock_data.push(0); // slot
        mock_data.extend_from_slice(&[0u8; 84]); // democmdinfo + in_seq + out_seq
        mock_data.extend_from_slice(&4i32.to_le_bytes()); // length: 4 bytes payload
        mock_data.extend_from_slice(&[1, 2, 3, 4]); // payload

        // Frame 1: cmd 3 (synctick), tick 0, slot 0
        mock_data.push(3);
        mock_data.extend_from_slice(&0i32.to_le_bytes());
        mock_data.push(0);

        // Frame 2: cmd 7 (stop)
        mock_data.push(7);

        let temp_dir = std::env::temp_dir();
        let test_path = temp_dir.join(format!(
            "test_sanitize_{}.dem",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        std::fs::write(&test_path, &mock_data).expect("test file must be writable");

        let modified = sanitize_demo_signon_ticks(&test_path).expect("Sanitization should succeed");
        assert_eq!(modified, 1, "Should have sanitized 1 packet tick");

        let updated_data = std::fs::read(&test_path).expect("test file must be readable");
        let _ = std::fs::remove_file(&test_path);

        // Check that the tick of Frame 0 is now 0
        let tick_frame0 = i32::from_le_bytes(
            updated_data[1073..1077]
                .try_into()
                .expect("test file must contain valid data"),
        );
        assert_eq!(tick_frame0, 0, "Signon packet tick should be zeroed");
    }
}
