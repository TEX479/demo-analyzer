> [!NOTE]
> This project is build using rust and requires `cargo` and friends, see
> [the rust website](https://www.rust-lang.org/) for how to get started.


## Compiling the code
This code can be compiled to a couple different binaries.
- [a command line interface (CLI)](cli.md) for analysing one or more demos
- [a graphical user interface (GUI)](gui.md) for analysis and configuration
- [a live analysis tool](live_analyzer.md) for automated analysis during matches

## Developing your own algorithms
Though most of the code you will need to interact with has doc comments that should explain how to use the code, the following section describes the structure of a cheat detection algorithm in bare text form. You can also view complete algorithms with supporting comments at `src/algorithms/` (reading the working code will probably get you further than this).

To write your own algorithm, you must implement the `CheatAlgorithm` trait. To do this, create a new file in the `src/algorithms/` directory. For example, if you want to detect 180 degree snaps, you might create `src/algorithms/viewangles_180degrees.rs`. In this file, you can define whatever structs, types etc you need to create your algorithm. At minimum, you need to implement some of the functions in `CheatAlgorithm`:

- `enabled_by_default(&self) -> bool` (REQUIRED): Should this algorithm run by default if -a isn't specified?
- `algorithm_name(&self) -> &'static str` (REQUIRED): Return your algorithm's name here. Best practice is to match the filename.
- `params(&mut self) -> Option<&mut AlgorithmParameters>`: Return any parameters your algorithm takes here (See `src/algorithms/write_to_file.rs` for example usage).
- `handled_messages(&self) -> Vec<MessageType>`: Return any message types that your algorithm needs to parse here. You can access the message objects by implementing `on_message`.
- `init(&mut self) -> Result<(), Error>`: Called before any other events. Use this instead of your object's constructor when performing any non-ephemeral actions e.g. modifying files.
- `on_tick(&mut self, state: &CheatAnalyserState, parser_state: &ParserState, jg: &JankGuard) -> Result<Vec<Detection>, Error>`: Called for each tick. A tick is triggered by the receipt of a NetTick message. The game state for the tick is passed in as a CheatAnalyserState.
- `on_message(&mut self, _message: &Message, _state: &CheatAnalyserState, _parser_state: &ParserState, _tick: DemoTick) -> Result<Vec<Detection>, Error>`: Called for each message (i.e. packet) in the demo. Which messages get parsed depends on what message types are being listened for (see `handled_messages`). NetTick messages trigger on_tick and on_message in that order.
- `finish(&mut self) -> Result<Vec<Detection>, Error>`: Called after all other events. Use for cleaning up or for aggregate analysis.

The functions that can return `Ok(Vec<Detection>)` are the main entry points for your algorithm. Your task is to process the incoming data and produce Detection objects for each event where cheating is suspected.

Basic information about the game state is provided as a CheatAnalyserState object via `CheatAlgorithm::on_tick` and `CheatAlgorithm::on_message`. To get more specific information out of the demo, use `handled_messages` and `on_message` to listen for specific message types. To understand the structure of CheatAnalyserState, try `cargo run --release -i "path/to/demo.dem" -a write_to_file` to write all the CheatAnalyserState objects to one large file as json. Each tick is written to a new line.

To register a detection, include it in the vector that's returned at the end of any detection function. Detections don't have to be returned in the same function call that the relevant data is introduced; you can store Detections elsewhere and return them all in CheatAlgorithm::finish() if you want, but make sure all the Detection objects you want to return are returned before the program terminates. This is a good pattern for aggregate detection methods e.g. crit hack detection, or for any situation where you need future information before confirming the detection.

Though it is best practice to return detections as soon as they are final, as tools like the `live_analyzer` rely on detecions to be returned before the match is over.

If you don't have any detections to return, just return an empty vector.

The `Error`s returned in some messages above are `anyhow::Error`s. As most of the handeling part ignores errors, they may be removed in the future.
