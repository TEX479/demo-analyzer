use crate::{
    base::{cheat_analyser_base::CheatAnalyser, demo_handler_base::CheatDemoHandler},
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
use tf_demo_parser::demo::{header::Header, packet::Packet, parser::RawPacketStream};

struct DemoFile {
    pub read_size: u64,
    pub parsed_size: usize,
    pub handler: CheatDemoHandler<CheatAnalyser>,
    pub bytes_in_buffer: Vec<u8>,
    detection_length: usize,
}

impl DemoFile {
    fn new(handler: CheatDemoHandler<CheatAnalyser>) -> Self {
        Self {
            read_size: 0,
            parsed_size: 0,
            handler,
            bytes_in_buffer: Vec::new(),
            detection_length: 0,
        }
    }

    pub fn handle_packet<'a>(&mut self, packet: Packet<'a>) -> anyhow::Result<Vec<Detection>> {
        self.handler
            .handle_packet(packet.clone())
            .context(format!("could not handle packet {:?}", packet))?;
        if self.detection_length < self.handler.analyser.detections.len() {
            let output = self.handler.analyser.detections.iter().skip(self.detection_length).cloned().collect::<Vec<_>>();
            self.detection_length = self.handler.analyser.detections.len().saturating_sub(1);
            Ok(output)
        } else {
            Ok(Vec::new())
        }
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
                // TODO: do some check to test if the file really did shorten
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
        let buffer_len_to_parse = bytes_new.len() * 8;
        demo_file.bytes_in_buffer.extend(bytes_new);

        let mut stream = BitReadStream::new(BitReadBuffer::new(&demo_file.bytes_in_buffer, LittleEndian));
        stream.skip_bits(demo_file.parsed_size).context(format!(
            "could not skip BitReadStream of bytes.len() = {} to desired bit {}",
            demo_file.bytes_in_buffer.len(),
            demo_file.parsed_size
        ))?;

        if demo_file.parsed_size == 0 {
            let header = Header::read(&mut stream).context("could not read Header")?;
            tracing::info!("new demo header: {:?}", header);
        }

        let mut packets: RawPacketStream = RawPacketStream::new(stream);
        loop {
            match packets.next(&demo_file.handler.state_handler) {
                Ok(Some(packet)) => {
                    demo_file
                        .handler
                        .handle_packet(packet.clone())
                        .context(format!("could not handle packet {:?}", packet))?;

                    // TODO: return detections instead of showing them here
                    if demo_file.handler.analyser.detections.len() > demo_file.detection_length {
                        demo_file
                            .handler
                            .analyser
                            .detections
                            .iter()
                            .skip(demo_file.detection_length)
                            .for_each(|detection| {
                                tracing::warn!("detection for player {} from algorithm {}", detection.player, detection.algorithm);
                                tracing::warn!("{}", detection.data);
                            });
                        demo_file.detection_length = demo_file.handler.analyser.detections.len();
                    }
                }
                Ok(None) => {
                    break;
                }
                Err(tf_demo_parser::ParseError::ReadError(BitError::NotEnoughData { requested, bits_left })) => {
                    return Err(anyhow::anyhow!(
                        "Tried to read packet but there were not enough bits. Requested: {}, Remaining: {}",
                        requested,
                        bits_left
                    ));
                }
                Err(e) => return Err(anyhow::anyhow!("Error reading demo packet: {}", e)),
            }
        }

        demo_file.parsed_size += buffer_len_to_parse;

        Ok(())
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
            .filter_map(|p| self.handle_event_for_path(p, event.kind, algorithm_getter).err())
            .collect::<Vec<anyhow::Error>>()
    }
}

pub fn live_analysis<A>(demo_path: PathBuf, mut algorithm_getter: A) -> anyhow::Result<Vec<CheatAnalyser>>
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

    // TODO: return cheat analyzers
    Ok(vec![])
}
