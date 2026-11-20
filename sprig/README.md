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

**Work with a list or a dictionary.** The list helpers take and return lists, and the
dictionary helpers take and return maps, so they compose without a template.

```rust
use neotales_sprig::{Value, call};

let list = call("list", &[Value::String("a".to_owned()), Value::String("b".to_owned())])?;
assert_eq!(list.render(), "[a b]");
let joined = call("join", &[Value::String("-".to_owned()), list])?;
assert_eq!(joined.render(), "a-b");

let dictionary = call("dict", &[Value::String("b".to_owned()), Value::Number(2.0)])?;
assert_eq!(dictionary.render(), "map[b:2]");
let keys = call("keys", &[dictionary])?;
assert_eq!(keys.render(), "[b]");
# Ok::<(), neotales_sprig::Error>(())
```

**Get a predictable `default`.** The helper has fixed arity two, taking a fallback and a
value. `false`, empty strings, empty lists and empty maps are empty; anything else is
returned unchanged. The variadic upstream form is not implemented.

```rust
use neotales_sprig::{Value, call};

let fallback = Value::String("local".to_owned());
let unset = Value::String(String::new());
assert_eq!(call("default", &[fallback.clone(), unset])?.render(), "local");
assert_eq!(
    call("default", &[fallback, Value::String("set".to_owned())])?.render(),
    "set"
);
# Ok::<(), neotales_sprig::Error>(())
```

**Take one group of helpers.** `group_names` lists the names in one section of the set,
grouped the way Go Sprig's `genericMap` groups them. A group Go has and this crate does not
— the crypto, random and network ones — is empty rather than a stub.

```rust
use neotales_sprig::{FunctionGroup, group_names};

assert!(group_names(FunctionGroup::Strings).contains(&"upper"));
assert!(group_names(FunctionGroup::Defaults).contains(&"default"));
assert!(group_names(FunctionGroup::Crypto).is_empty());
```

**Load the helpers into a template engine.** This crate never depends on an engine, so the
glue is yours: convert between the two value types, then register each name against a
closure that calls back in. The same `register` works for one group, for every group, and
for the hermetic set.

```rust
use std::collections::BTreeMap;

use neotales_gotmpl::{Error, FuncMap, Template, Value};
use neotales_sprig::{FunctionGroup, Value as SprigValue, group_names, hermetic_names};

// The two crates have their own value types, so the caller converts between
// them. Every variant maps onto the same variant.
fn to_sprig(value: &Value) -> SprigValue {
    match value {
        Value::String(value) => SprigValue::String(value.clone()),
        Value::Bool(value) => SprigValue::Bool(*value),
        Value::Number(value) => SprigValue::Number(*value),
        Value::Float(value) => SprigValue::Float(*value),
        Value::Null => SprigValue::Null,
        Value::List(values) => SprigValue::List(values.iter().map(to_sprig).collect()),
        Value::Map(values) => SprigValue::Map(
            values.iter().map(|(key, value)| (key.clone(), to_sprig(value))).collect(),
        ),
        Value::Object { display, members } => SprigValue::Object {
            display: display.clone(),
            members: members
                .iter()
                .map(|(key, value)| (key.clone(), to_sprig(value)))
                .collect(),
        },
        Value::Method { helper, receiver } => SprigValue::Method {
            helper: helper.clone(),
            receiver: Box::new(to_sprig(receiver)),
        },
    }
}

// The way back, for the value a helper returns.
fn to_engine(value: SprigValue) -> Value {
    match value {
        SprigValue::String(value) => Value::String(value),
        SprigValue::Bool(value) => Value::Bool(value),
        SprigValue::Number(value) => Value::Number(value),
        SprigValue::Float(value) => Value::Float(value),
        SprigValue::Null => Value::Null,
        SprigValue::List(values) => Value::List(values.into_iter().map(to_engine).collect()),
        SprigValue::Map(values) => Value::Map(
            values.into_iter().map(|(key, value)| (key, to_engine(value))).collect(),
        ),
        SprigValue::Object { display, members } => Value::Object {
            display,
            members: members
                .into_iter()
                .map(|(key, value)| (key, to_engine(value)))
                .collect(),
        },
        SprigValue::Method { helper, receiver } => Value::Method {
            helper,
            receiver: Box::new(to_engine(*receiver)),
        },
    }
}

// Registers every name in `names`. Additive: `insert` replaces only the name it
// is given, so a second call adds a second set of helpers.
fn register(functions: &mut FuncMap, names: &[&'static str]) {
    for &name in names {
        let helper = name.to_owned();
        // The string helpers expect their arguments already rendered, and it is
        // the helper set that knows which ones those are, not the engine.
        let rendered_arguments = neotales_sprig::takes_rendered_strings(name);
        functions.insert(name, move |arguments: &[Value]| {
            let arguments: Vec<SprigValue> = arguments
                .iter()
                .map(|argument| {
                    let value = to_sprig(argument);
                    if rendered_arguments {
                        SprigValue::String(value.render())
                    } else {
                        value
                    }
                })
                .collect();
            neotales_sprig::call(&helper, &arguments)
                .map(to_engine)
                .map_err(|error| Error::Function {
                    name: helper.clone(),
                    message: error.to_string(),
                })
        });
    }
}

// One group, registered on its own.
let mut functions = FuncMap::new();
register(&mut functions, group_names(FunctionGroup::Strings));

let values = BTreeMap::from([("name".to_owned(), "acme".to_owned())]);
let strings = Template::parse("{{ .name | upper }}")?.with_funcs(functions.clone());
assert_eq!(strings.render(&values)?, "ACME");

// The hermetic set: everything whose answer depends on nothing but its
// arguments. `now` and `ago` read the clock and are not in it.
let mut hermetic = FuncMap::new();
register(&mut hermetic, &hermetic_names());
assert!(!hermetic.contains("now"));
let repeated = Template::parse("{{ .name | upper }}")?.with_funcs(hermetic);
assert_eq!(repeated.render(&values)?, "ACME");

// A second group is added to the same map without replacing the first.
register(&mut functions, group_names(FunctionGroup::Defaults));
let defaults = BTreeMap::from([
    ("name".to_owned(), "acme".to_owned()),
    ("empty".to_owned(), String::new()),
]);
let both = Template::parse("{{ .name | upper }}-{{ default \"fallback\" .empty }}")?
    .with_funcs(functions);
assert_eq!(both.render(&defaults)?, "ACME-fallback");
# Ok::<(), neotales_gotmpl::Error>(())
```

`group_names` and `hermetic_names` are plain name lists, so the same loop registers
`FunctionGroup::DataStructures` (lists and dictionaries together) or every name in
`helper_names()`.

**Keep the environment out of it.** `env` and `expandenv` read the environment map on the
`Context` the caller passes to `call_with`, not the process environment. The sample above
uses `call`, which has no environment, so those two answer with the empty string; use
`call_with` and a `Context` built from your own map when a template needs them. No helper
opens a file, a socket or a process. `now` and `ago` read the clock and are the only two
that are not a pure function of their arguments, which is why `hermetic_names` leaves them
out.

## Licence 📜

[MIT](../LICENSE.md)
