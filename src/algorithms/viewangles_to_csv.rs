use std::collections::HashMap;
use std::fmt::Display;
use std::fs::File;
use std::io::Write;

use crate::base::cheat_analyser_base::{CheatAnalyserState, PlayerState};
use crate::lib::algorithm::{CheatAlgorithm, Detection};
use crate::lib::parameters::AlgorithmParameters;
use crate::util::helpers::geometry::{ViewAngle, viewangle_delta};
use crate::{dev_print, make_algorithm_params};
use anyhow::{Context, Error};
use tf_demo_parser::ParserState;

pub struct ViewAnglesToCSV {
    file: Option<File>,
    previous: Option<CheatAnalyserState>,
    history: Vec<Record>,
    write_batch_size: usize,
}

struct Record {
    tick: u32,
    name: String,
    steam_id: String,
    origin_x: f32,
    origin_y: f32,
    origin_z: f32,
    viewangle: f32,
    pitchangle: f32,
    va_delta: f32,
    pa_delta: f32,
}

impl Display for Record {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{},{},{},{},{},{},{},{},{},{}",
            self.tick, self.name, self.steam_id, self.origin_x, self.origin_y, self.origin_z, self.viewangle, self.pitchangle, self.va_delta, self.pa_delta
        )
    }
}

impl ViewAnglesToCSV {
    pub fn new() -> Self {
        let writer: ViewAnglesToCSV = ViewAnglesToCSV {
            file: None,
            previous: None,
            history: Vec::new(),
            write_batch_size: 2048,
        };
        writer
    }

    fn init_file(&mut self, file_path: &str) -> anyhow::Result<()> {
        self.file = Some(File::create(file_path).context(format!("could not create file {:?}", file_path))?);
        Ok(())
    }

    fn escape_csv_string(&self, input: &str) -> String {
        let mut output = String::new();
        output.push('"');

        for c in input.chars() {
            if c == '"' {
                output.push_str("\"\"");
            } else {
                output.push(c);
            }
        }

        output.push('"');
        output
    }
}

impl Default for ViewAnglesToCSV {
    fn default() -> Self {
        Self::new()
    }
}

impl CheatAlgorithm for ViewAnglesToCSV {
    fn enabled_by_default(&self) -> bool {
        false
    }

    fn algorithm_name(&self) -> &str {
        "viewangles_to_csv"
    }

    fn init(&mut self) -> Result<(), Error> {
        self.init_file("./output/viewangles_to_csv.csv")
            .context("could not initialize file './output/viewangles_to_csv.csv'")?;
        writeln!(
            self.file.as_mut().context("ViewAnglesToCSV.file is None but was expected to be Some")?,
            "tick,name,steam_id,origin_x,origin_y,origin_z,viewangle,pitchangle,va_delta,pa_delta"
        )
        .context("could not write header to output file")
    }

    fn on_tick(&mut self, state: &CheatAnalyserState, _: &ParserState) -> Result<Vec<Detection>, Error> {
        let ticknum = u32::from(state.tick);
        let players = &state.players;

        // In the vast majority of cases you will only want to iterate over players that are:
        // - In PVS (data is being sent to the client)
        // - Alive (you can't cheat if you're dead)
        // - Not a tf_bot (you can't convict a tf_bot)
        for player in players
            .iter()
            .filter(|p| p.in_pvs && p.state == PlayerState::Alive && p.info.as_ref().is_some_and(|info| info.steam_id != "BOT"))
        {
            let info = match &player.info {
                Some(info) => info,
                None => continue,
            };

            let name = self.escape_csv_string(&info.name);
            let origin_x = player.position.x;
            let origin_y = player.position.y;
            let origin_z = player.position.z;
            let viewangle = player.view_angle;
            let pitchangle = player.pitch_angle;
            let steam_id = &info.steam_id;

            let tick_delta = {
                if ticknum == 0 {
                    0
                } else {
                    ticknum - self.previous.as_ref().map_or(0, |pstate| pstate.tick.into())
                }
            };

            let (va_delta, pa_delta) = self.previous.as_ref().map_or((f32::NAN, f32::NAN), |prev_state| {
                match prev_state
                    .players
                    .iter()
                    .find(|p| p.in_pvs && p.state == PlayerState::Alive && p.info.as_ref().is_some_and(|i| i.steam_id == *steam_id))
                {
                    Some(prev_player) => {
                        let current_angle = ViewAngle::from(player);
                        let previous_angle = ViewAngle::from(prev_player);
                        viewangle_delta(current_angle, previous_angle, tick_delta)
                    }
                    None => (f32::NAN, f32::NAN),
                }
            });

            self.history.push(Record {
                tick: ticknum,
                name,
                steam_id: steam_id.clone(),
                origin_x,
                origin_y,
                origin_z,
                viewangle,
                pitchangle,
                va_delta,
                pa_delta,
            });
        }
        self.previous = Some(state.clone());

        Ok(vec![])
    }

    fn finish(&mut self) -> Result<Vec<Detection>, Error> {
        let dest = self.file.as_mut().context("ViewAnglesToCSV.file is None but was expected to be Some")?;
        let mut record_dict: HashMap<String, Vec<Record>> = HashMap::new();

        // This block is written to minimize memory usage while retaining performance.
        if !self.history.is_empty() {
            dev_print!("viewangles_to_csv: Writing csv output...");

            while let Some(record) = self.history.pop() {
                let steam_id = record.steam_id.clone();
                let records = record_dict.entry(steam_id).or_default();
                records.push(record);
            }

            // we want csv to be sorted by steamid...
            let mut sorted_steamids = record_dict.keys().cloned().collect::<Vec<String>>();

            sorted_steamids.sort();

            // ...and then by tick
            for steam_id in sorted_steamids {
                let records = record_dict.get_mut(&steam_id).context(format!("invalid steam_id3: {:?}", steam_id))?;
                records.sort_by_key(|a| a.tick);

                // write in batches to balance perf and memory usage
                for chunk in records.chunks(self.write_batch_size) {
                    writeln!(dest, "{}", chunk.iter().map(|r| r.to_string()).collect::<Vec<String>>().join("\n"))
                        .context(format!("could not write chunks to {:?}", dest))?;

                    dest.sync_data()?;
                }
            }
        }

        Ok(vec![])
    }

    fn params(&mut self) -> Option<AlgorithmParameters<'_>> {
        let params = make_algorithm_params!(self; [write_batch_size]);
        Some(params)
    }
}
