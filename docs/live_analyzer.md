# Live Analyzer

## Building
Building the live analyzer needs the flags `--bin=live_analyzer --features=live_analyzer`, so the build command looks something like this:
```sh
cargo build --bin=live_analyzer --features="live_analyzer" --release
```

This will place the binary into `<project root folder>/target/release/live_analyzer`

Running the binary will make it start checking the `<path to TF2>/tf/demos` folder and analyse any demos that are being written there. 
Any detections will be printed to the console as a warning.

## Launch Options
`-p` or `--params` followed by a path can be used to specify [a json file to configure the algorithms parameters](params.md).

`-d` or `--demo-path` followed by a path can be used to specify the folder which the program will watch for demos to analyse.

`-a` or `--algorithm` with an algorithm's name as next arg can be used to specify which algorithm's to run (use multiple `-a`/`--algorithm` calls for multiple algorithms).  
If not specified, only algorithms with `enabled_by_default()` being `true` are ran.
