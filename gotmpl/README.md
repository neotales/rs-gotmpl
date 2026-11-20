# neotales-gotmpl

Pure, safe Go `text/template` subset. The engine ships no helper library: the functions a
template calls come from a `FuncMap` the caller supplies, the way Go's own
`text/template` works.

By [x0craft](https://github.com/x0craft).

[![Neotales](https://github.com/neotales/brand/raw/main/assets/logo.png)](https://github.com/neotales/rs-gotmpl)

<!-- badges: crates.io, docs.rs, CI. Add once the first version exists. -->

## Install 📦

```sh
cargo add neotales-gotmpl
```

## What you can do 🔧

**Render a template.** `Template::parse` compiles the source once and keeps it free of
borrows. `render` takes a `BTreeMap<String, String>` and returns `Result<String, Error>`.
This works with nothing else installed: `.name` is the template language, not a helper.

```rust
use std::collections::BTreeMap;

use neotales_gotmpl::Template;

let template = Template::parse("Hello, {{ .name }}")?;
let values = BTreeMap::from([("name".to_owned(), "world".to_owned())]);
assert_eq!(template.render(&values)?, "Hello, world");
# Ok::<(), neotales_gotmpl::Error>(())
```

**Fail on a missing path.** A dot path the caller did not supply is `Error::MissingValue`.
The renderer never substitutes a silent empty string, so a typo in a template is a failed
render rather than a blank line in a config file.

```rust
use std::collections::BTreeMap;

use neotales_gotmpl::{Error, Template};

let template = Template::parse("Hello, {{ .name }}")?;
let error = template
    .render(&BTreeMap::new())
    .expect_err("no name was supplied");
assert_eq!(error, Error::MissingValue("name".to_owned()));
# Ok::<(), neotales_gotmpl::Error>(())
```

**Register a function.** A function is a closure that takes the evaluated arguments, in
source order and with any piped value last, and returns a `Value`. `FuncMap::insert` adds
one; `Template::with_funcs` attaches the map to a parsed template. There is no second
registration style: `len` and `index` are the language's own and need no map.

```rust
use std::collections::BTreeMap;

use neotales_gotmpl::{FuncMap, Template, Value};

let functions = FuncMap::new().with("shout", |arguments: &[Value]| {
    let Some(text) = arguments.first().and_then(Value::as_str) else {
        return Err(neotales_gotmpl::Error::Function {
            name: "shout".to_owned(),
            message: "shout needs a string".to_owned(),
        });
    };
    Ok(Value::String(text.to_uppercase()))
});

let template = Template::parse("Hello, {{ .name | shout }}")?.with_funcs(functions);
let values = BTreeMap::from([("name".to_owned(), "acme".to_owned())]);
assert_eq!(template.render(&values)?, "Hello, ACME");
# Ok::<(), neotales_gotmpl::Error>(())
```

**Pass typed data.** `render_typed` takes `BTreeMap<String, Value>`, where `Value` is this
crate's own type, so numbers, lists and maps keep their type instead of being flattened to
strings. A caller converting a value from another library maps like onto like.

**Bound the work.** `ParseOptions` caps source length and nesting depth. `RenderOptions`
caps output length, range iterations, function calls, named-template expansion, total
evaluations and nesting, and accepts a `Fn() -> bool` cancellation check. A limit that is
hit returns `Error::ResourceLimitExceeded` without echoing the rendered values.

**Stay off the host.** The crate reads no environment, network, filesystem or process
state, and it calls no clock. Everything a template produces is a function of the values
and the caller's function map.

**Add Sprig if you want it.** `neotales-sprig` is a separate crate and does not depend on
this one, which is the direction Go keeps between `text/template` and Sprig. Load it the
way the [`neotales-sprig` README](../sprig/README.md) shows: convert the two value types
once, register the names you want through `FuncMap::insert`, and `upper`, `default`, `list`
and the rest become available. `group_names` gives you one section of the set at a time and
`hermetic_names` gives you the repeatable subset, so no name list has to be typed by hand.
Nothing here assumes any of it is present, and a template that calls a name the map does
not hold fails with `Error::UnknownFunction` rather than rendering nothing.

## Licence 📜

[MIT](LICENSE.md)
