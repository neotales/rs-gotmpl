//! The glue from this crate's README, exercised end to end.
//!
//! `neotales-sprig` does not depend on a template engine, and this test is the
//! only place in this crate that needs one. It is a dev-dependency: the published
//! library graph stays one-way, and the code below is what the README's sample
//! says a caller writes.
//!
//! The two assertions that matter most are that registering a group gives a
//! template the helpers in that group, and that registering a second group adds
//! to the first rather than replacing it.

// `missing_docs` is a workspace lint aimed at the published API. A test binary
// has no public API to document.
#![allow(missing_docs)]

use std::collections::BTreeMap;

use neotales_gotmpl::{Error, FuncMap, Template, Value};
use neotales_sprig::{
    FunctionGroup, Value as SprigValue, group_names, helper_names, hermetic_names,
    takes_rendered_strings,
};

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

fn register(functions: &mut FuncMap, names: &[&'static str]) {
    for &name in names {
        let helper = name.to_owned();
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
            neotales_sprig::call(&helper, &arguments)
                .map(to_engine)
                .map_err(|error| Error::Function {
                    name: helper.clone(),
                    message: error.to_string(),
                })
        });
    }
}

fn values() -> BTreeMap<String, Value> {
    BTreeMap::from([
        ("name".to_owned(), Value::String("acme".to_owned())),
        ("empty".to_owned(), Value::String(String::new())),
    ])
}

/// One group registers the helpers in that group and nothing else.
#[test]
fn one_group_registers_its_own_helpers() {
    let mut functions = FuncMap::new();
    register(&mut functions, group_names(FunctionGroup::Strings));
    let template = Template::parse("{{ .name | upper }}")
        .expect("parses")
        .with_funcs(functions);
    assert_eq!(template.render_typed(&values()).expect("renders"), "ACME");

    // `default` is in the Defaults group, which was not registered.
    let mut strings_only = FuncMap::new();
    register(&mut strings_only, group_names(FunctionGroup::Strings));
    let template = Template::parse(r#"{{ default "fallback" .empty }}"#)
        .expect("parses")
        .with_funcs(strings_only);
    let error = template
        .render_typed(&values())
        .expect_err("the strings group does not include default");
    assert!(
        matches!(&error, Error::UnknownFunction(name) if name == "default"),
        "{error:?}"
    );
}

/// A second group adds to the map rather than replacing what is there.
#[test]
fn a_second_group_is_added_without_replacing_the_first() {
    let mut functions = FuncMap::new();
    register(&mut functions, group_names(FunctionGroup::Strings));
    assert!(functions.contains("upper"));
    assert!(!functions.contains("default"));

    register(&mut functions, group_names(FunctionGroup::Defaults));
    assert!(functions.contains("upper"), "the first group was lost");
    assert!(functions.contains("default"));

    let template = Template::parse(r#"{{ .name | upper }}-{{ default "fallback" .empty }}"#)
        .expect("parses")
        .with_funcs(functions);
    assert_eq!(
        template.render_typed(&values()).expect("renders"),
        "ACME-fallback"
    );
}

/// The hermetic set is every implemented helper that reads no clock and no
/// environment, and registering it is one call.
#[test]
fn the_hermetic_set_registers_in_one_call() {
    let mut functions = FuncMap::new();
    register(&mut functions, &hermetic_names());
    assert_eq!(functions.len(), hermetic_names().len());
    assert!(!functions.contains("now"));
    assert!(!functions.contains("ago"));
    assert!(!functions.contains("env"));

    let template = Template::parse("{{ .name | upper }}")
        .expect("parses")
        .with_funcs(functions);
    assert_eq!(template.render_typed(&values()).expect("renders"), "ACME");
}

/// Registering every name makes the whole set work, including the form that
/// reaches a helper through a value's method.
#[test]
fn every_implemented_name_can_be_registered() {
    let mut functions = FuncMap::new();
    register(&mut functions, &helper_names());
    let template = Template::parse(r#"{{ semver "1.2.3" | (semver "1.3.3").Compare }}"#)
        .expect("parses")
        .with_funcs(functions);
    assert_eq!(template.render_typed(&values()).expect("renders"), "1");
}

/// The data-structure group is the lists and the dictionaries together.
#[test]
fn the_data_structure_group_is_the_combined_set() {
    let mut functions = FuncMap::new();
    register(&mut functions, group_names(FunctionGroup::DataStructures));
    let template =
        Template::parse(r#"{{ list 1 2 | join "-" }}|{{ dict "a" 1 "b" 2 | keys | join "," }}"#)
            .expect("parses")
            .with_funcs(functions);
    assert_eq!(
        template.render_typed(&values()).expect("renders"),
        "1-2|a,b"
    );
}

/// A helper that refuses is reported as a function error carrying the helper's
/// own message, so a caller can tell which helper stopped the render.
#[test]
fn a_helper_refusal_carries_its_name_and_message() {
    let mut functions = FuncMap::new();
    register(&mut functions, group_names(FunctionGroup::Strings));
    let error = Template::parse(r#"{{ trim "a" "b" }}"#)
        .expect("parses")
        .with_funcs(functions)
        .render_typed(&values())
        .expect_err("trim takes one argument");
    match &error {
        Error::Function { name, message } => {
            assert_eq!(name, "trim");
            assert_eq!(
                message,
                "safe Sprig helper trim expects 1 arguments, received 2"
            );
        }
        other => panic!("expected a function error, got {other:?}"),
    }
}
