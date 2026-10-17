# Parameters
The way the code handles parameters has changed a bit and may change again and no doubt when changes happen people <sub>(me)</sub> will forget to update the docs, so if anything doesn't work, either
read the code or just ask someone who knows how this works.

## not so technical TL;DR
For the end user, the parameters of algorithms are just a json file like this:
```json
{
  "<algorithm name>": {
    "<parameter name>": {
      "<value type>": <value>
    }
  }
}
```
Where `<value type>` can be any of `Float32`, `Int32`, `Bool`, `Usize`, `Unsigned32` and `<value>` is the corresponding value.

Generate such a file using [the gui](gui.md).
