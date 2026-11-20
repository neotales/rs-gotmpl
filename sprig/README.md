# neotales-sprig

Deterministic, side-effect-free Sprig-compatible helpers for template engines.

By [x0craft](https://github.com/x0craft).

[![Neotales](https://github.com/neotales/brand/raw/main/assets/logo.png)](https://github.com/neotales/rs-gotmpl)

<!-- badges: crates.io, docs.rs, CI. Add once the first version exists. -->

## Install 📦

```sh
cargo add neotales-sprig
```

## What you can do 🔧

**Call a helper directly.** `call` takes the helper name and typed arguments and returns a
`Value`. Strings, numbers, lists and maps are handled without going through a template.

```rust
use neotales_sprig::{Value, call};

let value = call("upper", &[Value::String("acme".to_owned())])?;
assert_eq!(value.render(), "ACME");
# Ok::<(), neotales_sprig::Error>(())
```

**Render through a template.** `neotales-gotmpl` re-exports this crate's `Value` and
dispatches every filter through the same `call`, so a helper behaves the same called
directly or from inside `{{ ... }}`.

**Get a predictable `default`.** The helper has fixed arity two, taking a fallback and a
value. `false`, empty strings, empty lists and empty maps are empty; anything else is
returned unchanged. The variadic upstream form is not implemented.

**Keep the environment out of it.** `env` and `expandenv` read the environment map on the
`Context` the caller passes, not the process environment. No helper opens a file, a socket
or a process. `now` and `ago` read the clock and are the only two that are not a pure
function of their arguments.

## Licence 📜

[MIT](../LICENSE.md)
