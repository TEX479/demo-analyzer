# CLI
> [!NOTE]
> This is just a copy of the original docs.
> They are most likely outdated.

After setting up your Rust developer environment, check the program executes correctly with the command `cargo run -- -i "path/to/demo.dem"`. You can also add the `--release` flag after `cargo run` to dramatically speed up the program (2-10x), however this enables some safeguards that prevents panics from certain invalid states (e.g. u32 underflows), so be careful!

Example output: 
```Map: pl_borneo
Duration: 00:11:28.038 (45892 ticks)
User: megascatterbomb (part 2 soon)
Server: 117.120.11.35:6996
Starting analysis...
Processing tick 3238 (42654 remaining, 3237 tps)
Processing tick 5838 (40054 remaining, 2918 tps)
Processing tick 8750 (37142 remaining, 2915 tps)
Processing tick 12013 (33879 remaining, 3002 tps)
Processing tick 14829 (31063 remaining, 2964 tps)
Processing tick 16994 (28898 remaining, 2831 tps)
Processing tick 19277 (26615 remaining, 2752 tps)
Processing tick 21628 (24264 remaining, 2702 tps)
Processing tick 24571 (21321 remaining, 2729 tps)
Processing tick 27254 (18638 remaining, 2724 tps)
Processing tick 29753 (16139 remaining, 2703 tps)
Processing tick 33029 (12863 remaining, 2751 tps)
Processing tick 36511 (9381 remaining, 2807 tps)
Processing tick 39226 (6666 remaining, 2800 tps)
Processing tick 43498 (2394 remaining, 2898 tps)
Map: pl_borneo
Duration: 00:11:28.038 (45892 ticks)
User: megascatterbomb (part 2 soon)
Server: 117.120.11.35:6996
Done! (Processed 45892 ticks in 15.64 seconds averaging 2934.88 tps)
[]
```

The empty array at the bottom means there were no detections. If there were detections, they would look something like this:

```
[
  {
    "tick": 8466,
    "algorithm": "viewangles_180degrees",
    "player": 76561199776113179,
    "data": {
      "pa_delta": -180.0,
      "va_delta": -58.768341064453125
    }
  },
  {
    "tick": 8466,
    "algorithm": "viewangles_180degrees",
    "player": 76561199775364340,
    "data": {
      "pa_delta": -180.0,
      "va_delta": 33.07917404174805
    }
  },
  {
    "tick": 8469,
    "algorithm": "viewangles_180degrees",
    "player": 76561199774314308,
    "data": {
      "pa_delta": -180.0,
      "va_delta": 33.079193115234375
    }
  },
  {
    "tick": 8469,
    "algorithm": "viewangles_180degrees",
    "player": 76561199776113179,
    "data": {
      "pa_delta": 180.0,
      "va_delta": -134.0762424468994
    }
  }
]
```
These are real detections against real cheaters, but the Steam IDs have been substituted for now-deleted bot accounts.

In production, the `-q` flag is used to silence all debug info, leaving only the detection output in stdout. 

### Output

The output is a json array containing serialized Detection objects. The `Detection` struct is used to represent a single detection event. It contains the following fields:

- `tick: u64`: The tick number at which the detection occurred.
- `algorithm: String`: The name of the algorithm which produced the detection.
- `player: u64`: The Steam ID of the player who triggered the detection.
- `data: Value`: A JSON value containing any relevant data for the detection, such as what viewangles triggered the detection.

### Arguments

The program accepts the following arguments:

- `-a <algorithm> [-a <algorithm>]...`: Specify the algorithms to run. If not specified, the default algorithms are run.
- `-c`: Print the number of detections instead of details for every detection. Overridden by `-q`.
- `-h`: Print help information and exit.
- `-i <path>`: Specify the path to the demo file to analyse. **This argument is required.**
- `-p`: Provide a .json file with custom parameters. 
- `-q`: Silence all debug info, leaving only the detection output in stdout. Required for production use.
- `-Q`: Same as `-q`, but prettifies the output. Convenient for manual inspection of the output.