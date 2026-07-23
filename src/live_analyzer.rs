use crate::{
    base::{
        cheat_analyser_base::{CheatAnalyser, CheatAnalyserState},
        demo_handler_base::CheatDemoHandler,
    },
    lib::algorithm::{CheatAlgorithm, Detection},
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
use tf_demo_parser::demo::{header::Header, packet::Packet, parser::RawPacketStream};

pub struct DemoFile {
    pub read_size: u64,
    /// if the file's header has not been parsed yet
    pub handler: CheatDemoHandler<CheatAnalyser>,
    // TODO: add support for remainder from parsing
    pub header: Option<Header>,
    bytes_offset: usize,
    bytes: Vec<u8>,
    detection_length: usize,
}

impl DemoFile {
    fn new(handler: CheatDemoHandler<CheatAnalyser>) -> Self {
        Self {
            read_size: 0,
            handler,
            header: None,
            bytes_offset: 0,
            bytes: Vec::new(),
            detection_length: 0,
        }
    }

    pub fn process_next_chunk(&mut self) -> anyhow::Result<()> {
        self.process_next_chunk_and_keep_packets(|_| Option::<()>::None)?;
        Ok(())
    }

    pub fn process_next_chunk_and_keep_packets<ReturnType, Filter>(&mut self, keep_filter: Filter) -> anyhow::Result<Vec<ReturnType>>
    where
        Filter: Fn(&Packet) -> Option<ReturnType>,
    {
        let buffer = BitReadBuffer::new(&self.bytes, LittleEndian);
        let mut stream = BitReadStream::new(buffer);
        stream
            .set_pos(self.bytes_offset)
            .context(format!("could not skip to ahead to {:?}", self.bytes_offset))?;

        if self.header.is_none() {
            let header = Header::read(&mut stream).context("could not parse header")?;
            tracing::info!("{:?}", header);
            self.header = Some(header);
            self.bytes_offset = stream.pos();
        }

        let mut packets = RawPacketStream::new(stream);
        let mut kept_packets = Vec::new();
        loop {
            match packets.next(&self.handler.state_handler) {
                Ok(Some(packet)) => {
                    if let Some(rt) = keep_filter(&packet) {
                        kept_packets.push(rt);
                    }
                    self.handler
                        .handle_packet(packet.clone())
                        .context(format!("could not handle packet {:?}", packet))?;
                    self.bytes_offset = packets.pos();
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

        let handled_byte_count = self.bytes_offset / 8;
        self.bytes = self.bytes.iter().skip(handled_byte_count).copied().collect();
        self.bytes_offset -= handled_byte_count * 8;

        Ok(kept_packets)
    }

    pub fn add_bytes(&mut self, bytes: Vec<u8>) {
        self.bytes.extend(bytes);
    }
}

#[derive(Default)]
pub struct DemoFileManager {
    demos: HashMap<PathBuf, DemoFile>,
}

impl DemoFileManager {
    pub fn demo_at_path<A>(&mut self, path: &Path, algorithm_getter: &mut A) -> &mut DemoFile
    where
        A: FnMut() -> Vec<Box<dyn CheatAlgorithm + Send>>,
    {
        if !self.demos.contains_key(path) {
            let new_handler = CheatDemoHandler::with_analyser(CheatAnalyser::new(algorithm_getter()));
            let demo_file = DemoFile::new(new_handler);
            self.demos.insert(path.to_path_buf(), demo_file);
        }

        self.demos.get_mut(path).expect("we just inserted that element, it must exist")
    }

    // fn handle_event_for_path<A>(&mut self, path: PathBuf, event_kind: notify::EventKind, algorithm_getter: &mut A) -> anyhow::Result<()>
    // where
    //     A: Fn() -> Vec<Box<dyn CheatAlgorithm + Send>>,
    // {
    //     self.handle_event_for_path_and_collect_packets(path, event_kind, algorithm_getter, |_| Option::<()>::None)
    //         .map(|_| ())
    // }

    fn handle_event_for_path_and_collect_packets<A, F, R>(
        &mut self,
        path: PathBuf,
        event_kind: notify::EventKind,
        algorithm_getter: &mut A,
        filter: F,
    ) -> anyhow::Result<(Vec<R>, Vec<Detection>)>
    where
        A: Fn() -> Vec<Box<dyn CheatAlgorithm + Send>>,
        F: Fn(&Packet) -> Option<R>,
    {
        match event_kind {
            notify::EventKind::Create(_) => {}
            notify::EventKind::Modify(_) => {}
            _ => return Ok(Default::default()),
        }

        let demo_file = self.demo_at_path(&path, algorithm_getter);

        let file_length = {
            let meta = std::fs::metadata(&path).context(format!("could not aquire metadata for file {:?}", path))?;
            meta.len()
        };
        match file_length.cmp(&demo_file.read_size) {
            std::cmp::Ordering::Greater => {}
            std::cmp::Ordering::Equal => return Ok(Default::default()),
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
        demo_file.add_bytes(bytes_new);

        let ret_val = demo_file.process_next_chunk_and_keep_packets(filter)?;

        let detections = if demo_file.handler.analyser.detections.len() > demo_file.detection_length {
            let detections = demo_file.handler.analyser.detections.iter().skip(demo_file.detection_length).cloned().collect();
            demo_file.detection_length = demo_file.handler.analyser.detections.len();
            detections
        } else {
            Vec::new()
        };

        if !detections.is_empty() && crate::PRINT_DETECTIONS_LIVE.load(std::sync::atomic::Ordering::Relaxed) {
            for detection in detections.iter() {
                let player_string = match get_player_name(&demo_file.handler.analyser.state, detection.player) {
                    Some(name) => format!("{} ({:?})", detection.player, name),
                    None => {
                        tracing::warn!("could not get name for player with id {}", detection.player);
                        format!("{}", detection.player)
                    }
                };
                tracing::warn!(
                    "tick: {:06}; player: {};\talgorithm: {}\tdata: {:?}",
                    detection.tick,
                    player_string,
                    detection.algorithm,
                    detection.data,
                );
            }
        }

        Ok((ret_val, detections))
    }

    #[must_use]
    pub fn handle_event<A>(&mut self, event: notify::Event, algorithm_getter: &mut A) -> Vec<anyhow::Error>
    where
        A: Fn() -> Vec<Box<dyn CheatAlgorithm + Send>>,
    {
        match self.handle_event_and_collect_packets(event, algorithm_getter, |_| Option::<()>::None) {
            Ok(_) => Vec::new(),
            Err(e) => e,
        }
    }

    pub fn handle_event_and_collect_packets<A, F, R>(
        &mut self,
        event: notify::Event,
        algorithm_getter: &mut A,
        packet_filter: F,
    ) -> Result<(Vec<R>, Vec<Detection>), Vec<anyhow::Error>>
    where
        A: Fn() -> Vec<Box<dyn CheatAlgorithm + Send>>,
        F: Clone + Fn(&Packet) -> Option<R>,
    {
        let mut packets = Vec::new();
        let mut detections = Vec::new();
        let mut errs = Vec::new();
        for result in event.paths.into_iter().filter(|p| p.extension().is_some_and(|e| e == "dem")).map(|p| {
            self.handle_event_for_path_and_collect_packets(p.clone(), event.kind, algorithm_getter, packet_filter.clone())
                .context(format!("error with demo at {:?}", p))
        }) {
            match result {
                Ok((packets_new, detections_new)) => {
                    packets.extend(packets_new);
                    detections.extend(detections_new);
                }
                Err(e) => errs.push(e),
            }
        }
        if errs.is_empty() { Ok((packets, detections)) } else { Err(errs) }
    }
}

pub fn live_analysis<A>(demo_path: PathBuf, mut algorithm_getter: A) -> anyhow::Result<Vec<(PathBuf, CheatAnalyser)>>
where
    A: Fn() -> Vec<Box<dyn CheatAlgorithm + Send>>,
{
    let mut demos = DemoFileManager::default();

    let (tx, rx) = std::sync::mpsc::channel();
    let config = notify::Config::default().with_poll_interval(Duration::from_secs_f32(1.0));
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

    Ok(demos.demos.into_iter().map(|(path, demo)| (path, demo.handler.analyser)).collect())
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
