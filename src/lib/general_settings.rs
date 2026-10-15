use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use steamid_ng::SteamID;

use crate::{
    lib::{
        algorithm::Detection,
        parameters::{AlgorithmParameters, Config, SerializableParameters},
    },
    make_algorithm_params,
};

/// General program settings stored under "general_settings" in params.json.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GeneralSettings {
    /// Whether the global sliding window filter is enabled.
    pub global_sliding_window_enabled: bool,
    /// Span of the sliding window in ticks (default: 20000, ~5 minutes at 66.67 tps).
    pub sliding_window_ticks: u32,
    /// Minimum detections required within a sliding window for a player (or algorithm) to qualify.
    pub sliding_window_detections: usize,
    /// When true, the sliding window threshold is evaluated per algorithm individually.
    /// When false, detections from all algorithms for a player are aggregated into the window.
    pub per_algorithm: bool,
}

impl Default for GeneralSettings {
    fn default() -> Self {
        Self {
            global_sliding_window_enabled: true,
            sliding_window_ticks: 20000,
            sliding_window_detections: 2,
            per_algorithm: false,
        }
    }
}

impl GeneralSettings {
    /// Converts these settings into a `Parameters` map compatible with `params.json`.
    pub fn to_parameters(&mut self) -> AlgorithmParameters<'_> {
        make_algorithm_params!(self; [global_sliding_window_enabled, sliding_window_ticks, sliding_window_detections, per_algorithm])
    }

    /// Parses general settings from a `Parameters` map. Supports snake_case, spaces, and case-insensitivity.
    pub fn from_parameters(params: &SerializableParameters) -> Self {
        let mut settings = Self::default();

        for (k, v) in params {
            let key = k.to_lowercase().replace(' ', "_");
            match key.as_str() {
                "global_sliding_window_enabled" => {
                    if let Ok(b) = (*v).try_into() {
                        settings.global_sliding_window_enabled = b;
                    }
                }
                "sliding_window_ticks" => {
                    if let Ok(i) = (*v).try_into() {
                        settings.sliding_window_ticks = i;
                    }
                }
                "sliding_window_detections" => {
                    if let Ok(i) = (*v).try_into() {
                        settings.sliding_window_detections = i;
                    }
                }
                "per_algorithm" => {
                    if let Ok(b) = (*v).try_into() {
                        settings.per_algorithm = b;
                    }
                }
                _ => {}
            }
        }

        settings
    }

    /// Extracts `GeneralSettings` from a full config map (e.g., loaded from `params.json`).
    pub fn from_config(config: &Config) -> Self {
        for (key, params) in config {
            let normalized = key.to_lowercase().replace([' ', '-'], "_");
            if normalized == "general_settings" {
                return Self::from_parameters(params);
            }
        }
        Self::default()
    }

    /// Extracts `GeneralSettings` from an optional config map.
    pub fn from_config_opt(config: &Option<Config>) -> Self {
        match config {
            Some(cfg) => Self::from_config(cfg),
            None => Self::load_default(),
        }
    }

    /// Attempts to find and load `general_settings` from `params.json` in cwd or exe directory.
    /// Falls back to `GeneralSettings::default()` if not found or on error.
    pub fn load_default() -> Self {
        let candidate_path = if std::path::Path::new("params.json").exists() {
            Some(std::path::PathBuf::from("params.json"))
        } else if let Ok(mut exe) = std::env::current_exe() {
            exe.pop();
            let p = exe.join("params.json");
            if p.exists() { Some(p) } else { None }
        } else {
            None
        };

        if let Some(p) = candidate_path
            && let Ok(content) = std::fs::read_to_string(p)
            && let Ok(cfg) = serde_json::from_str::<Config>(&content)
        {
            return Self::from_config(&cfg);
        }

        Self::default()
    }

    /// Returns whether an algorithm is hardcoded as excluded from the global sliding window filter.
    ///
    /// Excluded algorithms include:
    /// 1. Algorithms used to detect manipulated demos or cheating recorders:
    ///    all demoswap, fidoo/tampered, all recorder related algorithms, lboxaimbot,
    ///    amalgam aimbot and triggerbot, j_backtrack, sv_cheats_bypass,
    ///    triggerbot double inputs, suspicious_motd
    /// 2. Algorithms with 100% accuracy:
    ///    oob pitch, invalid equip region
    pub fn is_algorithm_excluded(algorithm_name: &str) -> bool {
        is_excluded_from_sliding_window(algorithm_name)
    }

    /// Applies the global sliding window filter to a slice of detections.
    ///
    /// Semantics:
    /// - If `!global_sliding_window_enabled` or `sliding_window_detections <= 1`, all detections are retained.
    /// - Detections from excluded algorithms (100% accuracy or manipulated demo / recorder cheats) are ALWAYS retained.
    /// - When `per_algorithm == false`: If ANY sliding window of `sliding_window_ticks` contains >= `sliding_window_detections`
    ///   for a player across any algorithms (or if the player has an excluded algorithm detection), the player is considered qualified,
    ///   and ALL detections for that player are kept (including earlier and later ones). If no window reaches the threshold and no
    ///   excluded algorithms triggered, non-excluded detections for that player are hidden.
    /// - When `per_algorithm == true`: Excluded algorithms are always retained. Non-excluded algorithms are evaluated per
    ///   `(player, algorithm)` pair: if ANY window contains >= threshold detections for that specific algorithm on that player,
    ///   ALL detections for that pair are retained.
    pub fn filter_detections(&self, detections: &[Detection]) -> Vec<Detection> {
        if !self.global_sliding_window_enabled || self.sliding_window_detections <= 1 {
            return detections.to_vec();
        }

        let k = self.sliding_window_detections;
        let window_ticks = self.sliding_window_ticks;

        if !self.per_algorithm {
            // Group by player
            let mut player_ticks: HashMap<SteamID, Vec<u32>> = HashMap::new();
            let mut qualified_players = HashSet::new();

            for d in detections {
                if is_excluded_from_sliding_window(d.algorithm) {
                    // Excluded algorithm detections immediately qualify the player
                    qualified_players.insert(d.player);
                }
                player_ticks.entry(d.player).or_default().push(d.tick);
            }

            for (player, mut ticks) in player_ticks {
                if qualified_players.contains(&player) {
                    continue;
                }
                if ticks.len() < k {
                    continue;
                }
                ticks.sort_unstable();
                for i in (k - 1)..ticks.len() {
                    if ticks[i].saturating_sub(ticks[i - (k - 1)]) <= window_ticks {
                        qualified_players.insert(player);
                        break;
                    }
                }
            }

            detections
                .iter()
                .filter(|d| is_excluded_from_sliding_window(d.algorithm) || qualified_players.contains(&d.player))
                .cloned()
                .collect()
        } else {
            // Group by (player, algorithm)
            let mut group_ticks: HashMap<(SteamID, &str), Vec<u32>> = HashMap::new();
            for d in detections {
                if !is_excluded_from_sliding_window(d.algorithm) {
                    group_ticks.entry((d.player, d.algorithm)).or_default().push(d.tick);
                }
            }

            let mut qualified_groups = HashSet::new();
            for (group, mut ticks) in group_ticks {
                if ticks.len() < k {
                    continue;
                }
                ticks.sort_unstable();
                for i in (k - 1)..ticks.len() {
                    if ticks[i].saturating_sub(ticks[i - (k - 1)]) <= window_ticks {
                        qualified_groups.insert(group);
                        break;
                    }
                }
            }

            detections
                .iter()
                .filter(|d| is_excluded_from_sliding_window(d.algorithm) || qualified_groups.contains(&(d.player, d.algorithm)))
                .cloned()
                .collect()
        }
    }
}

/// Returns whether an algorithm is hardcoded as excluded from the global sliding window filter.
pub fn is_excluded_from_sliding_window(algorithm_name: &str) -> bool {
    if crate::lib::excluded_algorithms_stable::is_stable_excluded(algorithm_name) {
        return true;
    }

    #[cfg(has_experimental_exclusions)]
    {
        if crate::lib::excluded_algorithms_experimental::is_experimental_excluded(algorithm_name) {
            return true;
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn make_det(tick: u32, player: SteamID, algorithm: &'static str) -> Detection {
        Detection {
            tick,
            algorithm,
            player,
            data: json!({}),
        }
    }

    #[test]
    fn test_default_settings() {
        let settings = GeneralSettings::default();
        assert!(settings.global_sliding_window_enabled);
        assert_eq!(settings.sliding_window_ticks, 20000);
        assert_eq!(settings.sliding_window_detections, 2);
        assert!(!settings.per_algorithm);

        // When detections threshold is 2, single isolated detections are filtered
        let dets = vec![make_det(100, SteamID::from(1), "test_algo")];
        assert_eq!(settings.filter_detections(&dets).len(), 0);

        // 2 detections within 20000 ticks are kept
        let dets2 = vec![make_det(100, SteamID::from(1), "test_algo"), make_det(500, SteamID::from(1), "test_algo")];
        assert_eq!(settings.filter_detections(&dets2).len(), 2);
    }

    #[test]
    fn test_filter_isolated_false_flags() {
        let settings = GeneralSettings {
            global_sliding_window_enabled: true,
            sliding_window_ticks: 20000,
            sliding_window_detections: 2,
            per_algorithm: false,
        };

        // Player 1 has 2 isolated detections 50,000 ticks apart -> should be hidden
        let isolated = vec![make_det(1000, SteamID::from(1), "algo_a"), make_det(51000, SteamID::from(1), "algo_b")];
        assert_eq!(settings.filter_detections(&isolated).len(), 0);

        // Player 2 has 2 detections within 500 ticks -> both should be kept
        let cheater = vec![make_det(1000, SteamID::from(2), "algo_a"), make_det(1500, SteamID::from(2), "algo_b")];
        assert_eq!(settings.filter_detections(&cheater).len(), 2);
    }

    #[test]
    fn test_keeps_all_past_and_future_detections_when_qualified() {
        let settings = GeneralSettings {
            global_sliding_window_enabled: true,
            sliding_window_ticks: 20000,
            sliding_window_detections: 2,
            per_algorithm: false,
        };

        // Player 1 has:
        // - past detection at tick 100
        // - burst of 2 detections at tick 20000, 20500 (meets window of 20000)
        // - future isolated detection at tick 90000
        // ALL 4 detections must be kept!
        let dets = vec![
            make_det(100, SteamID::from(1), "algo_a"),
            make_det(20000, SteamID::from(1), "algo_b"),
            make_det(20500, SteamID::from(1), "algo_a"),
            make_det(90000, SteamID::from(1), "algo_c"),
        ];

        let filtered = settings.filter_detections(&dets);
        assert_eq!(filtered.len(), 4);
        assert_eq!(filtered[0].tick, 100);
        assert_eq!(filtered[1].tick, 20000);
        assert_eq!(filtered[2].tick, 20500);
        assert_eq!(filtered[3].tick, 90000);
    }

    #[test]
    fn test_per_algorithm_flag() {
        // With per_algorithm = false, two different algorithms on same player combine:
        let settings_global = GeneralSettings {
            global_sliding_window_enabled: true,
            sliding_window_ticks: 20000,
            sliding_window_detections: 2,
            per_algorithm: false,
        };

        let dets = vec![make_det(1000, SteamID::from(1), "algo_a"), make_det(1500, SteamID::from(1), "algo_b")];
        assert_eq!(settings_global.filter_detections(&dets).len(), 2);

        // With per_algorithm = true, different algorithms do NOT combine:
        let settings_per_algo = GeneralSettings {
            global_sliding_window_enabled: true,
            sliding_window_ticks: 20000,
            sliding_window_detections: 2,
            per_algorithm: true,
        };
        assert_eq!(settings_per_algo.filter_detections(&dets).len(), 0);

        // But if algo_a has 2 detections within window:
        let dets_matching = vec![
            make_det(1000, SteamID::from(1), "algo_a"),
            make_det(1500, SteamID::from(1), "algo_a"),
            make_det(1600, SteamID::from(1), "algo_b"), // algo_b only has 1
        ];
        let filtered = settings_per_algo.filter_detections(&dets_matching);
        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered[0].algorithm, "algo_a");
        assert_eq!(filtered[1].algorithm, "algo_a");
    }

    #[test]
    fn test_disabled_sliding_window() {
        let settings = GeneralSettings {
            global_sliding_window_enabled: false,
            sliding_window_ticks: 20000,
            sliding_window_detections: 2,
            per_algorithm: false,
        };

        let dets = vec![make_det(1000, SteamID::from(1), "algo_a")];
        assert_eq!(settings.filter_detections(&dets).len(), 1);
    }

    #[test]
    fn test_stable_excluded_algorithms_never_filtered() {
        let settings = GeneralSettings {
            global_sliding_window_enabled: true,
            sliding_window_ticks: 20000,
            sliding_window_detections: 3,
            per_algorithm: false,
        };

        // 100% accuracy algorithms (stable) should NEVER be filtered, even as a single detection
        let dets = vec![
            make_det(1000, SteamID::from(1), "nocrex/oob_pitch"),
            make_det(2000, SteamID::from(2), "fidoo/invalid_equip_region"),
        ];

        let filtered = settings.filter_detections(&dets);
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn test_stable_excluded_qualifies_player_globally() {
        let settings = GeneralSettings {
            global_sliding_window_enabled: true,
            sliding_window_ticks: 20000,
            sliding_window_detections: 3,
            per_algorithm: false,
        };

        // Player 1 has 1 oob_pitch detection and 1 un-excluded isolated detection.
        // Because oob_pitch is excluded (and qualifies player 1), all detections on player 1 are kept.
        let dets = vec![
            make_det(1000, SteamID::from(1), "nocrex/oob_pitch"),
            make_det(50000, SteamID::from(1), "some_aimbot"),
            make_det(1000, SteamID::from(2), "some_aimbot"), // player 2 has only 1 isolated non-excluded detection -> filtered out
        ];

        let filtered = settings.filter_detections(&dets);
        assert_eq!(filtered.len(), 2);
        assert!(filtered.iter().all(|d| d.player == SteamID::from(1)));
    }

    #[cfg(has_experimental_exclusions)]
    #[test]
    fn test_experimental_excluded_algorithms() {
        let settings = GeneralSettings {
            global_sliding_window_enabled: true,
            sliding_window_ticks: 20000,
            sliding_window_detections: 3,
            per_algorithm: true,
        };

        let dets = vec![
            make_det(1000, 1, "fidoo/tampered"),
            make_det(2000, 1, "fidoo/recorder_bhop"),
            make_det(3000, 1, "fidoo/demoswap_precision"),
            make_det(4000, 1, "sv_cheats_bypass"),
            make_det(5000, 1, "fidoo/lboxaimbot"),
            make_det(6000, 1, "fidoo/triggerbot_double_inputs"),
            make_det(7000, 1, "suspicious_motd"),
            make_det(8000, 1, "isolated_generic_algo"), // should be filtered out
        ];

        let filtered = settings.filter_detections(&dets);
        assert_eq!(filtered.len(), 7);
        assert!(!filtered.iter().any(|d| d.algorithm == "isolated_generic_algo"));
    }
}
