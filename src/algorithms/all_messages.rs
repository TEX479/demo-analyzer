use std::collections::HashMap;
use std::fs::{self, File};
use std::io::Write;

use anyhow::{Context, Error, Result};
use tf_demo_parser::demo::data::DemoTick;
use tf_demo_parser::demo::message::Message;
use tf_demo_parser::{MessageType, ParserState};

use crate::base::cheat_analyser_base::CheatAnalyserState;
use crate::dev_print;
use crate::lib::algorithm::{CheatAlgorithm, Detection};
use crate::lib::parameters::AlgorithmParameters;
use crate::util::nocrex::jankguard::JankGuard;

// header is not needed for this algorithm, but is included to serve as an example of how to handle the lifetimes.
#[allow(dead_code)]
pub struct AllMessages {
    msg_history: Vec<String>,
    file: Option<File>,
    first_write: bool,
    write_batch_size: usize,
}

impl AllMessages {
    fn write_messages_to_file(&mut self) -> Result<()> {
        let out = self.msg_history.join("\n");
        writeln!(self.file.as_mut().context("`file` field is None")?, "{}", out).context(format!("could not write to file {:?}", self.file))
    }

    pub fn init_file(&mut self, file_path: &str) -> Result<()> {
        self.file = Some(fs::File::create(file_path).context(format!("could not create file {}", file_path))?);
        Ok(())
    }

    pub fn new() -> AllMessages {
        AllMessages {
            msg_history: Vec::new(),
            file: None,
            first_write: true,
            write_batch_size: 2048,
        }
    }
}

impl Default for AllMessages {
    fn default() -> Self {
        Self::new()
    }
}

impl CheatAlgorithm for AllMessages {
    fn enabled_by_default(&self) -> bool {
        false
    }

    fn algorithm_name(&self) -> &str {
        "all_messages"
    }

    fn init(&mut self) -> Result<()> {
        dev_print!("Initializing AllMessages algorithm with write_batch_size = {}", self.write_batch_size);
        self.init_file("./output/all_messages.txt")
            .context("could not initialize file `./output/all_messages.txt`")
    }

    fn on_message(
        &mut self,
        message: &Message,
        _: &CheatAnalyserState,
        pstate: &ParserState,
        _jankguard: &JankGuard,
        tick: DemoTick,
    ) -> Result<Vec<Detection>> {
        let mut message = format!("({tick}) {:#?}", message);

        while let Some(start) = message.find("ClassId(") {
            let end = message[start + 9..].find(")").context("unmatched parenthesis for `ClassId`")? + start + 9;
            let id = message[start + 9..end].trim();
            let id = id.strip_suffix(",").unwrap_or(id);
            let id: u16 = id.parse().context(format!("invalid id {:?}", id))?;
            let class = pstate
                .server_classes
                .iter()
                .find(|sc| u16::from(sc.id) == id)
                .context("could not find class in `ClassId`")?;
            message.replace_range(start..=end, class.name.as_str());
        }

        self.msg_history.push(message);

        if self.msg_history.len() > self.write_batch_size {
            self.write_messages_to_file().context("could not write message history to file")?;

            self.msg_history.clear();
        }

        Ok(vec![])
    }

    fn handled_messages(&self) -> Result<Vec<MessageType>, bool> {
        Err(true)
    }

    fn finish(&mut self) -> Result<Vec<Detection>, Error> {
        if !self.msg_history.is_empty() {
            self.write_messages_to_file()?;
            self.msg_history.clear();
        }

        let _ = self.file.as_mut().context("self.file was None but was expected to be Some")?.flush();

        Ok(vec![])
    }

    fn params(&mut self) -> Option<AlgorithmParameters<'_>> {
        let mut params = HashMap::new();
        params.insert(
            "write_batch_size".to_owned(),
            crate::lib::parameters::AlgorithmParameter::Usize(&mut self.write_batch_size),
        );
        Some(params)
    }
}
