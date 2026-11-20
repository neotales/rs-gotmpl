//! Loading `neotales-sprig` into `neotales-gotmpl`.
//!
//! This crate is not published. It exists so that the two test suites and the
//! sample in `neotales-sprig`'s README share one copy of the value conversion:
//! neither library can hold it, because holding it would make one depend on the
//! other, and the whole point is that neither does.
//!
//! The one public item is [`call`], which has the shape
//! `FuncMap::register_names` and `FuncMap::register_group` take. A caller writes
//! the same match on the two value types, once.
#![allow(dead_code)]

use neotales_gotmpl::{Error, Value};
use neotales_sprig::{Value as SprigValue, takes_rendered_strings};

/// Calls one Sprig helper with engine values.
///
/// This is the closure `neotales_gotmpl`'s registration methods invoke for each
/// name: it converts the arguments, asks the helper set whether they want to be
/// rendered first, calls the helper, converts the answer back, and reports a
/// refusal as a message. A helper set with a different value type writes the same
/// function against its own type.
///
/// # Errors
///
/// Returns [`Error::Function`] with the helper's own message when the helper
/// refuses the call, or [`Error::UnknownFunction`] when the name is not one this
/// crate implements.
pub fn call(name: &str, arguments: &[Value]) -> Result<Value, Error> {
    if !neotales_sprig::helper_names().contains(&name) {
        return Err(Error::UnknownFunction(name.to_owned()));
    }
    // Which helpers want their arguments already rendered is a property of the
    // helper set, not of the engine, so it is decided here once per name.
    let rendered_arguments = takes_rendered_strings(name);
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
    neotales_sprig::call(name, &arguments)
        .map(from_sprig)
        .map_err(|error| Error::from(error.to_string()))
}

/// Converts an engine value into this helper set's value type.
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
fn from_sprig(value: SprigValue) -> Value {
    match value {
        SprigValue::String(value) => Value::String(value),
        SprigValue::Bool(value) => Value::Bool(value),
        SprigValue::Number(value) => Value::Number(value),
        SprigValue::Float(value) => Value::Float(value),
        SprigValue::Null => Value::Null,
        SprigValue::List(values) => Value::List(values.into_iter().map(from_sprig).collect()),
        SprigValue::Map(values) => Value::Map(
            values
                .into_iter()
                .map(|(key, value)| (key, from_sprig(value)))
                .collect(),
        ),
        SprigValue::Object { display, members } => Value::Object {
            display,
            members: members
                .into_iter()
                .map(|(key, value)| (key, from_sprig(value)))
                .collect(),
        },
        SprigValue::Method { helper, receiver } => Value::Method {
            helper,
            receiver: Box::new(from_sprig(*receiver)),
        },
    }
}
