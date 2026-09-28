things to do with the merged code:
- refactor
- refactor again
- make sure they return detections as soon as they are final
- make sure to use `valid_players()` and similar helpers
- remove `anyhow` from all possible places
- optimize (use `KeyValueVec` instead of `HashMap`s, etc)
- refactor again
- remove as many parsings as possible (for parameter values)
- check that no functionality of the old code was replaced

