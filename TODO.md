things to do with the merged code:
- refactor again
- make sure they return detections as soon as they are final
- make sure to use `valid_players()` and similar helpers
- remove `anyhow` from all possible places
- optimize (use `KeyValueVec` instead of `HashMap`s, etc)
- refactor again again
- remove as many parsings as possible (of parameter values)
- make sure that algos that do some pvs or alive check work properly

