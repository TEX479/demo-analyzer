use crate::{
    base::{
        cheat_analyser_base::{CheatAnalyser, CheatAnalyserState},
        demo_handler_base::CheatDemoHandler,
    },
    lib::algorithm::CheatAlgorithm,
};
use anyhow::Context;
use bitbuffer::{BitError, BitRead, BitReadBuffer, BitReadStream, LittleEndian};
use notify::{RecommendedWatcher, Watcher};
use std::{
    collections::HashMap,
    io::{Read, Seek},
    path::{Path, PathBuf},
    time::Duration,
};
use steamid_ng::SteamID;
use tf_demo_parser::demo::{header::Header, parser::RawPacketStream};

struct DemoFile {
    pub read_size: u64,
    /// if the file's header has not been parsed yet
    pub handler: CheatDemoHandler<CheatAnalyser>,
    // TODO: add support for remainder from parsing
    pub header: Option<Header>,
    handled_bits: usize,
    pub bytes: Vec<u8>,
    detection_length: usize,
}

impl DemoFile {
    fn new(handler: CheatDemoHandler<CheatAnalyser>) -> Self {
        Self {
            read_size: 0,
            handler,
            header: None,
            handled_bits: 0,
            bytes: Vec::new(),
            detection_length: 0,
        }
    }

    pub fn process_next_chunk(&mut self) -> anyhow::Result<()> {
        let buffer = BitReadBuffer::new(&self.bytes, LittleEndian);
        let mut stream = BitReadStream::new(buffer);
        stream
            .set_pos(self.handled_bits)
            .context(format!("could not skip to ahead to {:?}", self.handled_bits))?;

        if self.header.is_none() {
            let header = Header::read(&mut stream).context("could not parse header")?;
            tracing::info!("{:?}", header);
            self.header = Some(header);
            self.handled_bits = stream.pos();
        }

        let mut packets = RawPacketStream::new(stream);
        loop {
            match packets.next(&self.handler.state_handler) {
                Ok(Some(packet)) => {
                    self.handler
                        .handle_packet(packet.clone())
                        .context(format!("could not handle packet {:?}", packet))?;
                    self.handled_bits = packets.pos();
                }
                Ok(None) => break,
                Err(tf_demo_parser::ParseError::ReadError(BitError::NotEnoughData { requested, bits_left })) => {
                    tracing::warn!(
                        "Tried to read packet but there were not enough bits. Requested {}, Remaining {}",
                        requested,
                        bits_left
                    );
                    break;
                }
                Err(e) => {
                    return Err(anyhow::Error::new(e).context("could not get next packet from demo bytes"));
                }
            }
        }

        Ok(())
    }
}

#[derive(Default)]
struct DemoFileManager {
    demos: HashMap<PathBuf, DemoFile>,
}

impl DemoFileManager {
    pub fn demo_at_path(&mut self, path: &Path, analyzer_algorithms: Vec<Box<dyn CheatAlgorithm + Send>>) -> &mut DemoFile {
        if !self.demos.contains_key(path) {
            let new_handler = CheatDemoHandler::with_analyser(CheatAnalyser::new(analyzer_algorithms));
            let demo_file = DemoFile::new(new_handler);
            self.demos.insert(path.to_path_buf(), demo_file);
        }

        self.demos.get_mut(path).expect("we just inserted that element, it must exist")
    }

    fn handle_event_for_path<A>(&mut self, path: PathBuf, event_kind: notify::EventKind, algorithm_getter: &mut A) -> anyhow::Result<()>
    where
        A: FnMut() -> Vec<Box<dyn CheatAlgorithm + Send>>,
    {
        match event_kind {
            notify::EventKind::Create(_) => {}
            notify::EventKind::Modify(_) => {}
            _ => return Ok(()),
        }

        let demo_file = self.demo_at_path(&path, algorithm_getter());

        let file_length = {
            let meta = std::fs::metadata(&path).context(format!("could not aquire metadata for file {:?}", path))?;
            meta.len()
        };
        match file_length.cmp(&demo_file.read_size) {
            std::cmp::Ordering::Greater => {}
            std::cmp::Ordering::Equal => return Ok(()),
            std::cmp::Ordering::Less => {
                tracing::info!("file shortend. reopening {:?}", path);
                *demo_file = DemoFile::new(CheatDemoHandler::with_analyser(CheatAnalyser::new(algorithm_getter())));
            }
        }

        let bytes_new = {
            let mut file = std::fs::File::open(&path).context(format!("could not open file {:?}", path))?;
            file.seek(std::io::SeekFrom::Start(demo_file.read_size))
                .context(format!("could not seek to {} in {:?}", demo_file.read_size, path))?;
            let mut buffer = Vec::new();
            let byte_count_read = file.read_to_end(&mut buffer).context(format!("could not read file {:?}", path))? as u64;
            demo_file.read_size += byte_count_read;
            buffer
        };
        demo_file.bytes.extend(bytes_new);

        let ret_val = demo_file.process_next_chunk();

        // show detections
        if demo_file.handler.analyser.detections.len() > demo_file.detection_length {
            demo_file
                .handler
                .analyser
                .detections
                .iter()
                .skip(demo_file.detection_length)
                .for_each(|detection| {
                    let player_name = match get_player_name(&demo_file.handler.analyser.state, detection.player).map(|s| format!("{s:?}")) {
                        Some(s) => s,
                        None => {
                            tracing::warn!(
                                "could not get name for player with id {} from state {:?}",
                                detection.player,
                                demo_file.handler.analyser.state
                            );
                            "???".to_string()
                        }
                    };
                    tracing::warn!(
                        "detected {} for {} ({}). detecion data: {}",
                        detection.algorithm,
                        player_name,
                        detection.player,
                        detection.data
                    );
                });
            demo_file.detection_length = demo_file.handler.analyser.detections.len();
        }

        ret_val
    }

    #[must_use]
    pub fn handle_event<A>(&mut self, event: notify::Event, algorithm_getter: &mut A) -> Vec<anyhow::Error>
    where
        A: FnMut() -> Vec<Box<dyn CheatAlgorithm + Send>>,
    {
        event
            .paths
            .into_iter()
            .filter(|p| p.extension().is_some_and(|e| e == "dem"))
            .filter_map(|p| {
                self.handle_event_for_path(p.clone(), event.kind, algorithm_getter)
                    .context(format!("error with demo at {:?}", p))
                    .err()
            })
            .collect::<Vec<anyhow::Error>>()
    }
}

pub fn live_analysis<A>(demo_path: PathBuf, mut algorithm_getter: A) -> anyhow::Result<Vec<CheatAnalyser>>
where
    A: Fn() -> Vec<Box<dyn CheatAlgorithm + Send>>,
{
    let mut demos = DemoFileManager::default();

    let (tx, rx) = std::sync::mpsc::channel();
    let config = notify::Config::default().with_poll_interval(Duration::from_secs_f32(3.0));
    let mut watcher: RecommendedWatcher = Watcher::new(tx, config).context("could not construct watcher")?;
    watcher
        .watch(&demo_path, notify::RecursiveMode::Recursive)
        .context("could not watch demo folder")?;

    for msg in rx {
        let event = msg.context("error from filewatcher")?;
        demos
            .handle_event(event, &mut algorithm_getter)
            .into_iter()
            .for_each(|e| tracing::error!("{:?}", e));
    }

    // TODO: return cheat analyzers
    Ok(vec![])
}

pub fn get_player_name(state: &CheatAnalyserState, steam_id: u64) -> Option<&str> {
    state
        .players
        .iter()
        .rev()
        .filter_map(|p| p.info.as_ref())
        .find(|info| SteamID::from_steam3(&info.steam_id).is_ok_and(|id| u64::from(id) == steam_id))
        .map(|info| info.name.as_str())
}
