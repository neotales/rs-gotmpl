//! Loading the Sprig helper set into `neotales-gotmpl`.
//!
//! The two crates do not depend on each other: `neotales-sprig` is a function
//! set that never mentions a template engine, and `neotales-gotmpl` is an engine
//! that never mentions Sprig. The glue that joins them is this file, and it is
//! the same glue the `neotales-sprig` README shows: convert between the two value
//! types, and register each helper's name against a closure that calls
//! `neotales_sprig::call`.
//!
//! `neotales-sprig` is a dev-dependency of `neotales-gotmpl` for exactly this:
//! the tests need a real helper set to render the parity fixtures through. The
//! published libraries do not depend on each other.
#![allow(dead_code)]

use std::collections::BTreeMap;
use std::sync::Arc;

use neotales_gotmpl::{Error, FuncMap, Value};
use neotales_sprig::{
    FunctionGroup, Value as SprigValue, group_names, helper_names, hermetic_names,
    takes_rendered_strings,
};

/// Converts an engine value into a Sprig value.
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

/// Converts a Sprig value back into an engine value.
fn to_gotmpl(value: SprigValue) -> Value {
    match value {
        SprigValue::String(value) => Value::String(value),
        SprigValue::Bool(value) => Value::Bool(value),
        SprigValue::Number(value) => Value::Number(value),
        SprigValue::Float(value) => Value::Float(value),
        SprigValue::Null => Value::Null,
        SprigValue::List(values) => Value::List(values.into_iter().map(to_gotmpl).collect()),
        SprigValue::Map(values) => Value::Map(
            values
                .into_iter()
                .map(|(key, value)| (key, to_gotmpl(value)))
                .collect(),
        ),
        SprigValue::Object { display, members } => Value::Object {
            display,
            members: members
                .into_iter()
                .map(|(key, value)| (key, to_gotmpl(value)))
                .collect(),
        },
        SprigValue::Method { helper, receiver } => Value::Method {
            helper,
            receiver: Box::new(to_gotmpl(*receiver)),
        },
    }
}

/// Registers `names`, with `environment` available to `env` and `expandenv`.
fn register(
    names: &[&'static str],
    environment: Option<&Arc<BTreeMap<String, String>>>,
) -> FuncMap {
    let mut functions = FuncMap::new();
    for &name in names {
        let helper = name.to_owned();
        // Sprig's string helpers expect their arguments already rendered, and it
        // is the function set that knows which ones those are. The engine hands
        // over typed values and has no opinion.
        let rendered_arguments = takes_rendered_strings(name);
        let environment = environment.cloned();
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
            let context = match &environment {
                Some(environment) => neotales_sprig::Context::with_environment(environment),
                None => neotales_sprig::Context::default(),
            };
            neotales_sprig::call_with(&helper, &arguments, &context)
                .map(to_gotmpl)
                .map_err(|error| Error::Function {
                    name: helper.clone(),
                    message: error.to_string(),
                })
        });
    }
    functions
}

/// Every helper this crate implements.
pub(crate) fn sprig_functions() -> FuncMap {
    register(&helper_names(), None)
}

/// The helpers in one Go Sprig group.
pub(crate) fn group_functions(group: FunctionGroup) -> FuncMap {
    register(group_names(group), None)
}

/// The helpers whose answer depends on nothing but their arguments.
pub(crate) fn hermetic_functions() -> FuncMap {
    register(&hermetic_names(), None)
}

/// Every helper, with the template's environment supplied to `env` and
/// `expandenv`.
///
/// This is how a caller gives the environment-reading helpers a view that is not
/// the process environment: it closes over its own map when it builds the
/// function map, since the engine has no environment of its own.
pub(crate) fn sprig_functions_with_environment(environment: BTreeMap<String, String>) -> FuncMap {
    register(&helper_names(), Some(&Arc::new(environment)))
}
