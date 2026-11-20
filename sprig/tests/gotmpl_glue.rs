//! Loading Sprig helpers into the engine, exercised against the real engine.
//!
//! The two libraries do not depend on each other. The value conversion is
//! `adapter`, a workspace crate that is not published, so this file and
//! `neotales-gotmpl`'s tests share one copy of it. What is tested here is the
//! loading API the engine offers: a named group, a second group added to the
//! same map, the hermetic set, and a refusal that comes back as a message.

// `missing_docs` is a workspace lint aimed at the published API. A test binary
// has no public API to document.
#![allow(missing_docs)]

use std::collections::BTreeMap;

use neotales_gotmpl::{Error, FuncMap, Template, Value};
use neotales_sprig::{FunctionGroup, group_names, helper_names, hermetic_names};

fn values() -> BTreeMap<String, Value> {
    BTreeMap::from([
        ("name".to_owned(), Value::String("acme".to_owned())),
        ("empty".to_owned(), Value::String(String::new())),
    ])
}

/// One group registers the helpers in that group and nothing else, and the map
/// reports what was loaded.
#[test]
fn one_group_registers_its_own_helpers() {
    let mut functions = FuncMap::new();
    functions
        .register_group(
            "strings",
            group_names(FunctionGroup::Strings),
            adapter::call,
        )
        .expect("the group name is not empty");

    let recorded: Vec<&str> = functions
        .group("strings")
        .expect("the group was recorded")
        .iter()
        .map(String::as_str)
        .collect();
    assert_eq!(recorded, group_names(FunctionGroup::Strings));
    assert_eq!(functions.groups().collect::<Vec<_>>(), ["strings"]);

    let template = Template::parse("{{ .name | upper }}")
        .expect("parses")
        .with_funcs(functions);
    assert_eq!(template.render_typed(&values()).expect("renders"), "ACME");
}

/// `default` is in the Defaults group, which was not registered, so it is an
/// unknown function rather than a silent empty value.
#[test]
fn a_group_only_registers_its_own_names() {
    let mut functions = FuncMap::new();
    functions
        .register_group(
            "strings",
            group_names(FunctionGroup::Strings),
            adapter::call,
        )
        .expect("the group name is not empty");
    let error = Template::parse(r#"{{ default "fallback" .empty }}"#)
        .expect("parses")
        .with_funcs(functions)
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
    functions
        .register_group(
            "strings",
            group_names(FunctionGroup::Strings),
            adapter::call,
        )
        .expect("the group name is not empty");
    assert!(functions.contains("upper"));
    assert!(!functions.contains("default"));

    functions
        .register_group(
            "defaults",
            group_names(FunctionGroup::Defaults),
            adapter::call,
        )
        .expect("the group name is not empty");
    assert!(functions.contains("upper"), "the first group was lost");
    assert!(functions.contains("default"));
    assert_eq!(
        functions.groups().collect::<Vec<_>>(),
        ["defaults", "strings"]
    );

    let template = Template::parse(r#"{{ .name | upper }}-{{ default "fallback" .empty }}"#)
        .expect("parses")
        .with_funcs(functions);
    assert_eq!(
        template.render_typed(&values()).expect("renders"),
        "ACME-fallback"
    );
}

/// A group with no name could not be reported, so it is refused. Nothing is
/// registered and nothing panics.
#[test]
fn an_empty_group_name_is_refused() {
    let mut functions = FuncMap::new();
    for group in ["", "   "] {
        let error = functions
            .register_group(group, group_names(FunctionGroup::Strings), adapter::call)
            .expect_err("an empty group name is refused");
        assert_eq!(error, Error::EmptyGroupName);
    }
    assert!(functions.is_empty());
    assert!(functions.groups().next().is_none());
}

/// The hermetic set is every implemented helper that reads no clock and no
/// environment, and registering it is one call.
#[test]
fn the_hermetic_set_registers_in_one_call() {
    let mut functions = FuncMap::new();
    functions
        .register_group("hermetic", &hermetic_names(), adapter::call)
        .expect("the group name is not empty");
    assert_eq!(functions.len(), hermetic_names().len());
    assert!(!functions.contains("now"));
    assert!(!functions.contains("ago"));
    assert!(!functions.contains("env"));

    let template = Template::parse("{{ .name | upper }}")
        .expect("parses")
        .with_funcs(functions);
    assert_eq!(template.render_typed(&values()).expect("renders"), "ACME");
}

/// `register_names` loads a list without recording a group, and the form that
/// reaches a helper through a value's method works once it is registered.
#[test]
fn every_implemented_name_can_be_registered() {
    let mut functions = FuncMap::new();
    functions.register_names(&helper_names(), adapter::call);
    assert_eq!(functions.len(), helper_names().len());
    assert!(functions.groups().next().is_none());

    let template = Template::parse(r#"{{ semver "1.2.3" | (semver "1.3.3").Compare }}"#)
        .expect("parses")
        .with_funcs(functions);
    assert_eq!(template.render_typed(&values()).expect("renders"), "1");
}

/// The data-structure group is the lists and the dictionaries together.
#[test]
fn the_data_structure_group_is_the_combined_set() {
    let mut functions = FuncMap::new();
    functions
        .register_group(
            "data_structures",
            group_names(FunctionGroup::DataStructures),
            adapter::call,
        )
        .expect("the group name is not empty");
    let template =
        Template::parse(r#"{{ list 1 2 | join "-" }}|{{ dict "a" 1 "b" 2 | keys | join "," }}"#)
            .expect("parses")
            .with_funcs(functions);
    assert_eq!(
        template.render_typed(&values()).expect("renders"),
        "1-2|a,b"
    );
}

/// A helper that refuses stops the render with the helper's own message.
#[test]
fn a_helper_refusal_carries_the_helper_message() {
    let mut functions = FuncMap::new();
    functions
        .register_group(
            "strings",
            group_names(FunctionGroup::Strings),
            adapter::call,
        )
        .expect("the group name is not empty");
    let error = Template::parse(r#"{{ trim "a" "b" }}"#)
        .expect("parses")
        .with_funcs(functions)
        .render_typed(&values())
        .expect_err("trim takes one argument");
    assert_eq!(
        error,
        Error::Function {
            name: String::new(),
            message: "safe Sprig helper trim expects 1 arguments, received 2".to_owned(),
        }
    );
    assert_eq!(
        error.to_string(),
        "safe Sprig helper trim expects 1 arguments, received 2"
    );
}
