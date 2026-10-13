use std::fs::{self, File};
use std::io::Write;

use anyhow::{Context, Error};
use tf_demo_parser::ParserState;

use crate::base::cheat_analyser_base::CheatAnalyserState;
use crate::lib::algorithm::{CheatAlgorithm, Detection};
use crate::lib::parameters::AlgorithmParameters;
use crate::make_algorithm_params;
use crate::util::nocrex::jankguard::JankGuard;

#[allow(dead_code)]
pub struct WriteToFile {
    state_history: Vec<CheatAnalyserState>,
    file: Option<File>,
    first_write: bool,
    write_batch_size: usize,
}

impl WriteToFile {
    fn write_states_to_file(&mut self) -> anyhow::Result<()> {
        if self.first_write {
            self.first_write = false;
        } else {
            writeln!(self.file.as_mut().context("WriteToFile.file was expected to be Some but is None")?, ",")
                .context(format!("could not write to {:?}", self.file))?;
        }

        let out = self
            .state_history
            .iter()
            .map(|j| serde_json::to_string(&j).context(format!("could not serialize {:?}", j)))
            .collect::<Result<Vec<String>, _>>()?
            .join(",\n");

        write!(self.file.as_mut().context("WriteToFile.file was expected to be Some but is None")?, "{}", out)
            .context(format!("could not write to {:?}", self.file))?;
        self.state_history.clear();
        Ok(())
    }

    pub fn init_file(&mut self, file_path: &str) -> anyhow::Result<()> {
        self.file = Some(fs::File::create(file_path).context(format!("could not create file {:?}", file_path))?);
        Ok(())
    }

    pub fn new() -> anyhow::Result<WriteToFile> {
        let write_batch_size = 1024;
        Ok(WriteToFile {
            state_history: Vec::with_capacity(write_batch_size as usize),
            file: None,
            first_write: true,
            write_batch_size: 1024,
        })
    }
}

// lifetimes not needed for this algorithm, but is included to serve as an example of how to handle them.
impl CheatAlgorithm for WriteToFile {
    fn enabled_by_default(&self) -> bool {
        false
    }

    fn algorithm_name(&self) -> &'static str {
        "write_to_file"
    }

    fn init(&mut self) -> Result<(), Error> {
        self.init_file("./output/write_to_file.json")
            .context("could not initialize file at './output/write_to_file.json'")?;

        writeln!(self.file.as_mut().context("WriteToFile.file was expected to be Some but is None")?, "[")
            .context(format!("could not write to {:?}", self.file))
    }

    fn on_tick(&mut self, state: &CheatAnalyserState, _: &ParserState, _jg: &JankGuard) -> Result<Vec<Detection>, Error> {
        self.state_history.push(state.clone());
        if self.state_history.len() > self.write_batch_size {
            self.write_states_to_file().context("could not write states to file")?;
        }

        Ok(vec![])
    }

    fn finish(&mut self) -> Result<Vec<Detection>, Error> {
        if !self.state_history.is_empty() {
            self.write_states_to_file().context("could not write states to file")?;
        }

        writeln!(self.file.as_mut().context("WriteToFile.file was expected to be Some but is None")?, "\n]")
            .context(format!("could not write to file {:?}", self.file))?;
        let _ = self.file.as_mut().map(|f| f.flush());

        Ok(vec![])
    }

    fn params(&mut self) -> Option<AlgorithmParameters<'_>> {
        Some(make_algorithm_params!(self; [write_batch_size]))
    }
}
