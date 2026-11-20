# rs-gotmpl

Two crates that are independent by default:

- `neotales-gotmpl` renders a pure, safe subset of Go `text/template`, and has no helper
  library of its own. The functions a template calls are registered by the caller.
- `neotales-sprig` supplies the deterministic Sprig helpers, and its default build knows
  nothing about a template engine. Its optional `gotmpl` feature adds the glue that loads
  the helpers into the engine — off by default, and never required by either crate.

That is the direction Go keeps: `text/template` does not depend on Sprig. A program that
wants both turns the feature on, and the [`neotales-sprig` README](sprig/README.md) has the
sample that does it.

By [x0craft](https://github.com/x0craft).

[![Neotales](https://github.com/neotales/brand/raw/main/assets/logo.png)](https://github.com/neotales/rs-gotmpl)

| Crate | Directory | README |
| --- | --- | --- |
| `neotales-gotmpl` | `gotmpl/` | [gotmpl/README.md](gotmpl/README.md) |
| `neotales-sprig` | `sprig/` | [sprig/README.md](sprig/README.md) |
