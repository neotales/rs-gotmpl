# rs-gotmpl

Two crates that do not depend on each other:

- `neotales-gotmpl` renders a pure, safe subset of Go `text/template`, and has no helper
  library of its own. The functions a template calls are registered by the caller.
- `neotales-sprig` supplies the deterministic Sprig helpers, and knows nothing about a
  template engine.

That is the direction Go keeps: `text/template` does not depend on Sprig, and Sprig does not
depend on `text/template`. A program that wants both loads the helpers into the engine with
`register_group`, and the [`neotales-sprig` README](sprig/README.md) has the sample that
does it.

By [x0craft](https://github.com/x0craft).

[![Neotales](https://github.com/neotales/brand/raw/main/assets/logo.png)](https://github.com/neotales/rs-gotmpl)

| Crate | Directory | README |
| --- | --- | --- |
| `neotales-gotmpl` | `gotmpl/` | [gotmpl/README.md](gotmpl/README.md) |
| `neotales-sprig` | `sprig/` | [sprig/README.md](sprig/README.md) |

`adapter/` is not published: it holds the value conversion between the two, which neither
library can own without depending on the other. The tests of both crates and the sprig
README sample compile against it.
