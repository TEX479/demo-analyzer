# Live Analyzer

## Building
Building the live analyzer needs the flags `--bin=live_analyzer --features=live_analyzer`, so the build command looks something like this:
```sh
cargo build --bin=live_analyzer --features="live_analyzer" --release
```

This will place the binary into `<project root folder>/target/release/live_analyzer`

Running the binary will make it start checking the `<path to TF2>/tf/demos` folder and analyse any demos that are being written there. 
Any detections will be printed to the console as a warning.

The binary can take one optional argument, being [a json parameter file](params.md):
```sh
live_analyzer params.json
```
The file can be generated using [the gui](gui.md) or by just creating [a matching json file](params.md).

