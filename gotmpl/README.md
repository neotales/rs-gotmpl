# neotales-gotmpl

Pure, safe Go `text/template` subset with deterministic Sprig helpers.

By [x0craft](https://github.com/x0craft).

[![Neotales](https://github.com/neotales/brand/raw/main/assets/logo.png)](https://github.com/neotales/rs-gotmpl)

<!-- badges: crates.io, docs.rs, CI. Add once the first version exists. -->

## Install 📦

```sh
cargo add neotales-gotmpl
```

## What you can do 🔧

**Render a template.** `Template::parse` compiles the source once and keeps it free of
borrows. `render` takes `BTreeMap<String, String>` and returns `Result<String, Error>`.
Filters such as `upper` come from the bundled Sprig set.

```rust
use std::collections::BTreeMap;

use neotales_gotmpl::Template;

let template = Template::parse("Hello, {{ .name | upper }}!")?;
let values = BTreeMap::from([("name".to_owned(), "acme".to_owned())]);
assert_eq!(template.render(&values)?, "Hello, ACME!");
# Ok::<(), neotales_gotmpl::Error>(())
```

**Pass typed data.** `render_typed` takes `BTreeMap<String, Value>`, where `Value` is
re-exported from `neotales-sprig`, so numbers, lists and maps keep their type instead of
being flattened to strings.

**Bound the work.** `ParseOptions` caps source length and nesting depth. `RenderOptions`
caps output length, range iterations, helper calls, named-template expansion, total
evaluations and nesting, and accepts a `Fn() -> bool` cancellation check. A limit that is
hit returns `Error::ResourceLimitExceeded` without echoing the rendered values.

**Fail on a missing path.** A dot path the caller did not supply is `Error::MissingValue`.
The renderer never substitutes a silent empty string, so a typo in a template is a failed
render rather than a blank line in a config file.

**Stay off the host.** The crate reads no environment, network, filesystem or process
state. Helpers are pure functions of their arguments; `now` and `ago` are the only two
that read the clock.

## Licence 📜

[MIT](LICENSE.md)
