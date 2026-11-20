//! Loading this crate's helpers into `neotales-gotmpl`.
//!
//! Available with the `gotmpl` feature, which is off by default: a default build
//! of this crate names no template engine, and the two crates stay independent in
//! that direction. With the feature on, this module is the glue — the conversion
//! between the two value types, the per-helper argument policy, and the error
//! mapping — so a caller does not have to keep a copy of it.
//!
//! [`register`] takes any name list. [`register_group`] and [`register_hermetic`]
//! call it with the lists [`group_names`](crate::group_names) and
//! [`hermetic_names`](crate::hermetic_names) produce, and every one of them adds
//! to the map it is given rather than replacing it.

use neotales_gotmpl::{Error, FuncMap, Value};

use crate::{
    FunctionGroup, Value as SprigValue, group_names, hermetic_names, takes_rendered_strings,
};

/// Registers every name in `names` into `functions`.
///
/// Each name is registered against a closure that calls [`call`](crate::call)
/// with the arguments the engine evaluated. A function that refuses becomes
/// [`Error::Function`], carrying the name it was registered under and the
/// helper's own message.
///
/// The map is added to, never cleared: a name already registered under the same
/// name is replaced, and every other name is left alone. Registering a second
/// group therefore keeps the first.
///
/// ```
/// use neotales_gotmpl::FuncMap;
/// use neotales_sprig::{FunctionGroup, group_names, register};
///
/// let mut functions = FuncMap::new();
/// register(&mut functions, group_names(FunctionGroup::Encoding));
/// assert!(functions.contains("b64enc"));
/// assert!(!functions.contains("upper"));
/// ```
pub fn register(functions: &mut FuncMap, names: &[&str]) {
    for &name in names {
        let helper = name.to_owned();
        // Which helpers want their arguments already rendered is a property of
        // the helper set, not of the engine, so the decision is made here once
        // per helper rather than by the engine for every call.
        let rendered_arguments = takes_rendered_strings(name);
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
            crate::call(&helper, &arguments)
                .map(to_engine)
                .map_err(|error| Error::Function {
                    name: helper.clone(),
                    message: error.to_string(),
                })
        });
    }
}

/// Registers the helpers in one Go Sprig group.
///
/// The group's names come from [`group_names`](crate::group_names), so a group
/// Go has and this crate does not is simply empty. Calling this twice with two
/// groups gives a map holding both.
///
/// ```
/// use neotales_gotmpl::{FuncMap, Template};
/// use neotales_sprig::{FunctionGroup, register_group};
///
/// let mut functions = FuncMap::new();
/// register_group(&mut functions, FunctionGroup::Strings);
/// register_group(&mut functions, FunctionGroup::Defaults);
///
/// let values = [("name".to_owned(), "acme".to_owned())].into_iter().collect();
/// let rendered = Template::parse("{{ .name | upper }}")
///     .expect("parses")
///     .with_funcs(functions)
///     .render(&values)
///     .expect("renders");
/// assert_eq!(rendered, "ACME");
/// ```
pub fn register_group(functions: &mut FuncMap, group: FunctionGroup) {
    register(functions, group_names(group));
}

/// Registers the implemented helpers whose answer depends on nothing but their
/// arguments.
///
/// This is [`hermetic_names`](crate::hermetic_names): every helper here except
/// the ones that read a clock or an environment, so two runs over the same input
/// produce the same bytes.
///
/// ```
/// use neotales_gotmpl::FuncMap;
/// use neotales_sprig::register_hermetic;
///
/// let mut functions = FuncMap::new();
/// register_hermetic(&mut functions);
/// assert!(functions.contains("upper"));
/// assert!(!functions.contains("now"));
/// assert!(!functions.contains("env"));
/// ```
pub fn register_hermetic(functions: &mut FuncMap) {
    register(functions, &hermetic_names());
}

/// Converts an engine value into this crate's value type.
fn to_sprig(value: &Value) -> SprigValue {
    match value {
        Value::String(value) => SprigValue::String(value.clone()),
        Value::Bool(value) => SprigValue::Bool(*value),
        Value::Number(value) => SprigValue::Number(*value),
        Value::Float(value) => SprigValue::Float(*value),
        Value::Null => SprigValue::Null,
        Value::List(values) => SprigValue::List(values.iter().map(to_sprig).collect()),
        Value::Map(values) => SprigValue::Map(
            values
                .iter()
                .map(|(key, value)| (key.clone(), to_sprig(value)))
                .collect(),
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

/// Converts a helper's result back into the engine's value type.
fn to_engine(value: SprigValue) -> Value {
    match value {
        SprigValue::String(value) => Value::String(value),
        SprigValue::Bool(value) => Value::Bool(value),
        SprigValue::Number(value) => Value::Number(value),
        SprigValue::Float(value) => Value::Float(value),
        SprigValue::Null => Value::Null,
        SprigValue::List(values) => Value::List(values.into_iter().map(to_engine).collect()),
        SprigValue::Map(values) => Value::Map(
            values
                .into_iter()
                .map(|(key, value)| (key, to_engine(value)))
                .collect(),
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
