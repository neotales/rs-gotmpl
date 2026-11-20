//! The `gotmpl` feature, exercised against the real engine.
//!
//! The glue itself lives in `src/gotmpl.rs`; this file is the only place in the
//! crate that needs a template engine, and it needs one only to prove that a
//! registered helper actually renders. `neotales-gotmpl` is the optional
//! dependency that the feature turns on, so the default build of this crate never
//! sees it.

// This file needs the engine, so the whole binary is behind the feature that
// brings it in. Without the feature the file compiles to an empty test binary.
#![cfg(feature = "gotmpl")]
// `missing_docs` is a workspace lint aimed at the published API. A test binary
// has no public API to document.
#![allow(missing_docs)]

use std::collections::BTreeMap;

use neotales_gotmpl::{Error, FuncMap, Template, Value};
use neotales_sprig::{FunctionGroup, helper_names, register, register_group, register_hermetic};

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
    register_group(&mut functions, FunctionGroup::Strings);
    let template = Template::parse("{{ .name | upper }}")
        .expect("parses")
        .with_funcs(functions);
    assert_eq!(template.render_typed(&values()).expect("renders"), "ACME");

    // `default` is in the Defaults group, which was not registered.
    let mut strings_only = FuncMap::new();
    register_group(&mut strings_only, FunctionGroup::Strings);
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
    register_group(&mut functions, FunctionGroup::Strings);
    assert!(functions.contains("upper"));
    assert!(!functions.contains("default"));

    register_group(&mut functions, FunctionGroup::Defaults);
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
    register_hermetic(&mut functions);
    assert_eq!(functions.len(), neotales_sprig::hermetic_names().len());
    assert!(!functions.contains("now"));
    assert!(!functions.contains("ago"));
    assert!(!functions.contains("env"));

    let template = Template::parse("{{ .name | upper }}")
        .expect("parses")
        .with_funcs(functions);
    assert_eq!(template.render_typed(&values()).expect("renders"), "ACME");
}

/// `register` takes any name list, including the full set, and the form that
/// reaches a helper through a value's method works once it is registered.
#[test]
fn every_implemented_name_can_be_registered() {
    let mut functions = FuncMap::new();
    register(&mut functions, &helper_names());
    assert_eq!(functions.len(), helper_names().len());
    let template = Template::parse(r#"{{ semver "1.2.3" | (semver "1.3.3").Compare }}"#)
        .expect("parses")
        .with_funcs(functions);
    assert_eq!(template.render_typed(&values()).expect("renders"), "1");
}

/// The data-structure group is the lists and the dictionaries together.
#[test]
fn the_data_structure_group_is_the_combined_set() {
    let mut functions = FuncMap::new();
    register_group(&mut functions, FunctionGroup::DataStructures);
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
    register_group(&mut functions, FunctionGroup::Strings);
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
