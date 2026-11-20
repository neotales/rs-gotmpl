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

**Load the helpers into a template engine.** An engine loads a helper set by naming
it once and dispatching on the name through `call`, so the loading step is the engine's
own API. Here it is `neotales-gotmpl`, which takes a group name of your choosing, the
names `group_names` returns, and one closure that converts the values and calls in.

The conversion is the same for every group, so it is written once: `adapter::call` in
`adapter/src/lib.rs` of this repository is a `match` on the nine value variants each way,
and it is what these samples compile against. It cannot live in either library, because
holding it would make one depend on the other.

```rust
use std::collections::BTreeMap;

use neotales_gotmpl::{FuncMap, Template};
use neotales_sprig::{FunctionGroup, group_names, hermetic_names};

// One group, under a name the caller chooses.
let mut functions = FuncMap::new();
functions.register_group("strings", group_names(FunctionGroup::Strings), adapter::call)?;

let values = BTreeMap::from([("name".to_owned(), "acme".to_owned())]);
let strings = Template::parse("{{ .name | upper }}")?.with_funcs(functions.clone());
assert_eq!(strings.render(&values)?, "ACME");

// The hermetic set: everything whose answer depends on nothing but its
// arguments. `now` and `ago` read the clock and are not in it.
let mut hermetic = FuncMap::new();
hermetic.register_group("hermetic", &hermetic_names(), adapter::call)?;
assert!(!hermetic.contains("now"));
let repeated = Template::parse("{{ .name | upper }}")?.with_funcs(hermetic);
assert_eq!(repeated.render(&values)?, "ACME");

// A second group adds to the same map without replacing the first, and the map
// reports what each group put there.
functions.register_group("defaults", group_names(FunctionGroup::Defaults), adapter::call)?;
let loaded: Vec<&str> = functions
    .group("defaults")
    .expect("the group was recorded")
    .iter()
    .map(String::as_str)
    .collect();
assert!(loaded.contains(&"default"));

let defaults = BTreeMap::from([
    ("name".to_owned(), "acme".to_owned()),
    ("empty".to_owned(), String::new()),
]);
let both =
    Template::parse("{{ .name | upper }}-{{ default \"fallback\" .empty }}")?.with_funcs(functions);
assert_eq!(both.render(&defaults)?, "ACME-fallback");
# Ok::<(), neotales_gotmpl::Error>(())
```

`group_names`, `hermetic_names` and `helper_names` are plain name lists, so a group, the
repeatable subset, and the whole set are all one `register_group` or `register_names` call
away. A group Go has and this crate does not — the crypto, random and network ones — is
empty rather than a stub, so nothing registers a name `call` would refuse.

**Keep the environment out of it.** `env` and `expandenv` read the environment map on the
`Context` passed to `call_with`, and `call` passes none, so through this loading step they
answer with the empty string. A template that needs a value from the environment should
find it in the values map (`.env.NAME`) rather than read the host. That is the guarantee
that matters: no helper reads the process environment, opens a file, a socket or a
process. `now` and `ago` read the clock and are the only two that are not a pure function
of their arguments, which is why `hermetic_names` leaves them out.


## Licence 📜

[MIT](../LICENSE.md)
