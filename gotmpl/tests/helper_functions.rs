//! The engine with a real helper set loaded, the way a caller loads one.
//!
//! `neotales-gotmpl` has no helper library. Every function a template calls
//! beyond the language's own `len` and `index` comes from the [`FuncMap`] the
//! caller attaches, and `support` is the glue that fills one from
//! `neotales-sprig` — the same glue the `neotales-sprig` README documents.
//!
//! These cases live here rather than in the crate's unit tests because they are
//! about the two crates working together, which a unit test of the engine should
//! not need. The unit tests in `src/` use no helper set at all.

// `missing_docs` is a workspace lint aimed at the published API. A test binary
// has no public API to document, so the lint firing here is noise rather than a
// finding.
#![allow(missing_docs)]

mod support;

use std::collections::BTreeMap;

use neotales_gotmpl::{Error, MissingValueBehavior, RenderOptions, Template, Value};
use neotales_sprig::FunctionGroup;
use support::{
    group_functions, hermetic_functions, sprig_functions, sprig_functions_with_environment,
};

fn render(source: &str, root: &BTreeMap<String, Value>) -> Result<String, Error> {
    Template::parse(source)
        .map(|template| template.with_funcs(sprig_functions()))
        .and_then(|template| template.render_typed(root))
}

fn render_with_options(
    source: &str,
    root: &BTreeMap<String, Value>,
    options: RenderOptions<'_>,
) -> Result<String, Error> {
    Template::parse(source)
        .map(|template| template.with_funcs(sprig_functions()))
        .and_then(|template| template.render_typed_with_options(root, options))
}

/// The two missing-value behaviors, and the idiom each one is for.
///
/// `Error` is right for a configuration file: a typo should stop the render
/// rather than become an empty value a container is then started with. `Null` is
/// what Go's own `text/template` does and is what
/// `{{ .env.OPTIONAL | default "x" }}` needs — under `Error` that idiom cannot be
/// written at all, because the failure happens at the field access before
/// `default` ever sees a value.
#[test]
fn a_missing_field_is_an_error_or_null_by_configuration() {
    let source = "{{ .env.OPTIONAL | default \"fallback\" }}";
    // The root carries `env` as an empty map, which is the shape a Compose
    // template sees: the namespace always exists, and a variable inside it may
    // not.
    let root = BTreeMap::from([("env".to_owned(), Value::Map(BTreeMap::new()))]);

    let error = render_with_options(source, &root, RenderOptions::default())
        .expect_err("the default must stop on a value that is not there");
    assert!(
        matches!(error, Error::MissingValue(ref path) if path == "env.OPTIONAL"),
        "{error}"
    );

    let lenient = RenderOptions {
        missing_value: MissingValueBehavior::Null,
        ..RenderOptions::default()
    };
    assert_eq!(
        render_with_options(source, &root, lenient).expect("a helper can answer for a null"),
        "fallback"
    );
}

/// A value that *is* set is unaffected by the choice.
#[test]
fn the_behavior_does_not_change_a_value_that_is_present() {
    let source = "{{ .env.SET | default \"fallback\" }}";
    let root = BTreeMap::from([(
        "env".to_owned(),
        Value::Map(BTreeMap::from([(
            "SET".to_owned(),
            Value::String("actual".to_owned()),
        )])),
    )]);
    for missing_value in [MissingValueBehavior::Error, MissingValueBehavior::Null] {
        let options = RenderOptions {
            missing_value,
            ..RenderOptions::default()
        };
        assert_eq!(
            render_with_options(source, &root, options).expect("renders"),
            "actual",
            "{missing_value:?} must not affect a value that is set"
        );
    }
}

#[test]
fn evaluates_safe_helpers_and_pipelines_without_host_access() {
    let root = BTreeMap::from([(
        "env".to_owned(),
        Value::Map(BTreeMap::from([(
            "NAME".to_owned(),
            Value::String(" acme ".to_owned()),
        )])),
    )]);
    assert_eq!(
        render("{{ .env.NAME | trim | upper | quote }}", &root).expect("renders"),
        "\"ACME\""
    );

    let root = BTreeMap::from([(
        "env".to_owned(),
        Value::Map(BTreeMap::from([(
            "VALUE".to_owned(),
            Value::String(String::new()),
        )])),
    )]);
    assert_eq!(
        render("{{ default \"fallback\" .env.VALUE }}", &root).expect("renders"),
        "fallback"
    );
}

/// `env` reads the environment the caller gives it and nothing else.
///
/// The engine has no environment of its own, so a caller that wants `env` closes
/// over the map it wants read when it builds the function map. That is the same
/// guarantee as before — a rendered file depends only on its inputs, never on the
/// process environment — with the decision moved to the caller.
#[test]
fn env_reads_the_environment_the_caller_supplies() {
    let functions =
        sprig_functions_with_environment(BTreeMap::from([("NAME".to_owned(), "neo".to_owned())]));
    let rendered = Template::parse(r#"{{ env "NAME" }}|{{ env "ABSENT" }}"#)
        .expect("parses")
        .with_funcs(functions)
        .render(&BTreeMap::new())
        .expect("renders");
    // A name that is not set is the empty string, as Go's sprig `env` is,
    // rather than an error: a template that probes for an optional value must
    // not fail because the value is absent.
    assert_eq!(rendered, "neo|");
}

/// A bare token in a template has its Go literal type.
///
/// Before this, every bare token was text, so `empty false` reported that
/// `false` was not empty and `kindOf 42` reported `string`. A helper that
/// inspects a value cannot tell a boolean from the four characters `true` if
/// both arrive as text.
#[test]
fn a_bare_token_carries_its_go_literal_type() {
    let cases = [
        ("{{ kindOf true }}", "bool"),
        ("{{ kindOf 42 }}", "int"),
        ("{{ kindOf 4.2 }}", "float64"),
        ("{{ empty false }}", "true"),
        ("{{ empty 0 }}", "true"),
        ("{{ int true }}", "1"),
        ("{{ float64 true }}", "1"),
        ("{{ 0x10 }}", "16"),
        ("{{ 0o10 }}", "8"),
        ("{{ 0b101 }}", "5"),
        ("{{ 010 }}", "8"),
        ("{{ 1_000 }}", "1000"),
        // A quoted number is text, which is the distinction a caller needs.
        ("{{ kindOf \"42\" }}", "string"),
        // A word that is not a number stays text.
        ("{{ kindOf alpha }}", "string"),
    ];
    for (source, expected) in cases {
        let rendered = render(source, &BTreeMap::new())
            .unwrap_or_else(|error| panic!("{source} should render: {error}"));
        assert_eq!(rendered, expected, "{source}");
    }
}

/// A negative literal keeps its sign.
///
/// The sign was applied twice on the way in once, which turned `-3` into `3`
/// and made `until -3` count up.
#[test]
fn a_negative_literal_keeps_its_sign() {
    for (source, expected) in [
        ("{{ until -3 }}", "[0 -1 -2]"),
        ("{{ until 3 }}", "[0 1 2]"),
        ("{{ untilStep 1 6 2 }}", "[1 3 5]"),
    ] {
        let rendered = render(source, &BTreeMap::new())
            .unwrap_or_else(|error| panic!("{source} should render: {error}"));
        assert_eq!(rendered, expected, "{source}");
    }
}

#[test]
fn renders_nested_conditionals_with_go_template_truthiness() {
    let root = BTreeMap::from([(
        "env".to_owned(),
        Value::Map(BTreeMap::from([
            ("ENABLED".to_owned(), Value::Bool(true)),
            ("NAME".to_owned(), Value::String("acme".to_owned())),
        ])),
    )]);
    let rendered = render(
        "{{ if .env.ENABLED }}yes {{ if contains \"acm\" .env.NAME }}acm{{ else }}other{{ end }}{{ else }}no{{ end }}",
        &root,
    )
    .expect("renders");
    assert_eq!(rendered, "yes acm");
}

#[test]
fn decodes_go_quoted_and_raw_strings() {
    let root = BTreeMap::from([(
        "value".to_owned(),
        Value::String("neo\nraw text".to_owned()),
    )]);
    assert_eq!(
        render(
            "{{ contains \"neo\\n\" .value }}|{{ contains `raw text` .value }}",
            &root
        )
        .expect("renders"),
        "true|true"
    );
}

#[test]
fn contains_uses_sprig_substring_first_in_calls_and_pipelines() {
    let root = BTreeMap::from([("value".to_owned(), Value::String(" Acme ".to_owned()))]);
    assert_eq!(
        render(
            "{{ contains \"Acm\" .value }}|{{ .value | contains \"Acm\" }}|{{ contains .value \"Acm\" }}",
            &root
        )
        .expect("renders"),
        "true|true|false"
    );
}

#[test]
fn decodes_go_escape_forms_with_exact_lengths() {
    let root = BTreeMap::from([("value".to_owned(), Value::String("ABCD\n".to_owned()))]);
    assert_eq!(
        render(
            "{{ contains \"\\x41\\102\\u0043\\U00000044\\n\" .value }}|{{ .value }}|{{ \"\\xC3\\xA9\" }}",
            &root
        )
        .expect("renders"),
        "true|ABCD\n|é"
    );

    for source in [r#"{{ "\x4" }}"#, r#"{{ "\12" }}"#, r#"{{ "\q" }}"#] {
        assert!(Template::parse(source).is_err(), "accepted {source:?}");
    }
}

/// A helper called with the wrong number of arguments is the helper's refusal,
/// not the engine's: the engine passes the arguments and reports what came back.
#[test]
fn a_helper_called_with_the_wrong_arity_is_reported() {
    let error = render("{{ first 1 2 3 4 5 }}", &BTreeMap::new())
        .expect_err("the helper must refuse the call");
    assert!(
        matches!(&error, Error::Function { name, .. } if name == "first"),
        "{error:?}"
    );
    assert_eq!(
        error.to_string(),
        "template function first failed: safe Sprig helper first expects 1 arguments, \
         received 5"
    );
}

/// One group is enough to register one section of the helper set.
#[test]
fn a_group_registers_only_its_own_helpers() {
    let root = BTreeMap::from([("name".to_owned(), Value::String("acme".to_owned()))]);
    let strings = Template::parse("{{ upper .name }}")
        .expect("parses")
        .with_funcs(group_functions(FunctionGroup::Strings));
    assert_eq!(strings.render_typed(&root).expect("renders"), "ACME");

    // The list helpers were not registered, so a call to one is reported rather
    // than quietly succeeding.
    let lists = Template::parse("{{ list 1 2 }}")
        .expect("parses")
        .with_funcs(group_functions(FunctionGroup::Strings));
    let error = lists
        .render_typed(&BTreeMap::new())
        .expect_err("the strings group does not include `list`");
    assert!(
        matches!(&error, Error::UnknownFunction(name) if name == "list"),
        "{error:?}"
    );
}

/// Go registers lists and dictionaries as one "Data Structures" section, and a
/// caller can ask for that combined set in one name.
#[test]
fn the_data_structure_group_is_lists_and_dicts_together() {
    let rendered =
        Template::parse(r#"{{ list 1 2 | join "-" }}|{{ dict "a" 1 "b" 2 | keys | join "," }}"#)
            .expect("parses")
            .with_funcs(group_functions(FunctionGroup::DataStructures))
            .render_typed(&BTreeMap::new())
            .expect("renders");
    assert_eq!(rendered, "1-2|a,b");
}

/// The hermetic set renders what it promises and omits what it excludes.
#[test]
fn the_hermetic_set_omits_the_helpers_that_read_a_clock_or_an_environment() {
    let root = BTreeMap::from([("name".to_owned(), Value::String("acme".to_owned()))]);
    let hermetic = Template::parse("{{ upper .name }}")
        .expect("parses")
        .with_funcs(hermetic_functions());
    assert_eq!(hermetic.render_typed(&root).expect("renders"), "ACME");

    for helper in ["now", "ago", "env"] {
        let template = Template::parse(&format!("{{{{ {helper} \"1\" }}}}"))
            .expect("parses")
            .with_funcs(hermetic_functions());
        let error = template
            .render_typed(&BTreeMap::new())
            .expect_err("a non-hermetic helper is not in the hermetic set");
        assert!(
            matches!(&error, Error::UnknownFunction(name) if name == helper),
            "{helper}: {error:?}"
        );
    }
}

/// Grouped arguments are the parser's job and must work through something other
/// than the engine's own builtins. These use real helpers so the whole path —
/// tokenizing, argument evaluation, the function map, the helper — is covered.
mod grouped_argument_tests {
    use std::collections::BTreeMap;

    use neotales_gotmpl::{Template, Value};

    use crate::support::sprig_functions;

    fn render(source: &str) -> String {
        let mut root = BTreeMap::new();
        root.insert("name".to_owned(), Value::String("neo".to_owned()));
        Template::parse(source)
            .expect("parses")
            .with_funcs(sprig_functions())
            .render_typed(&root)
            .expect("renders")
    }

    /// Go's grammar allows an argument to be a parenthesised pipeline, and
    /// passing a composed value that way is ordinary template code. Before this
    /// was supported the argument tokenized as three tokens, so the helper was
    /// handed the wrong operands and rendered the wrong answer rather than
    /// reporting an error.
    #[test]
    fn a_parenthesised_pipeline_is_one_argument() {
        assert_eq!(render(r#"{{ upper (trim " go " | repeat 2) }}"#), "GOGO");
        assert_eq!(render(r#"{{ len (splitList "-" "a-b-c") }}"#), "3");
    }

    #[test]
    fn a_group_may_contain_spaces_and_nested_groups() {
        assert_eq!(
            render(r#"{{ upper (trim "  neo  ") }}{{ lower (upper "AB") }}"#),
            "NEOab"
        );
    }

    #[test]
    fn a_group_may_be_nested() {
        assert_eq!(render(r#"{{ upper (repeat (len "ab") "z") }}"#), "ZZ");
    }

    /// A `(` with no partner is a malformed action, not a literal.
    #[test]
    fn an_unbalanced_parenthesis_is_rejected() {
        for source in [r#"{{ upper (trim " go " }}"#, r#"{{ ( "x" }}"#] {
            assert!(
                Template::parse(source).is_err(),
                "{source} should not parse"
            );
        }
    }

    /// A `(` inside a quoted string is text, not a group.
    #[test]
    fn a_parenthesis_inside_a_string_is_not_a_group() {
        assert_eq!(render(r#"{{ upper "a(b" }}"#), "A(B");
    }

    /// A group in the *first* position is a value, not a call to a helper whose
    /// name happens to be the group.
    ///
    /// The reference asserts this form itself,
    /// `SprigUpstreamParityTests.cs:519-522`:
    ///
    /// ```csharp
    /// [InlineData(
    ///     "{{ range (tuple 1 2 3 4 5 6 7 8 | mustChunk 3) }}" +
    ///     "{{ . | join \"-\" }}|{{ end }}",
    ///     "1-2-3|4-5-6|7-8|")]
    /// ```
    ///
    /// Before this was supported the action parsed as a call to a helper named
    /// `(tuple 1 2 3 4 5 6 7 8 | mustChunk 3)` and rendered
    /// `unsupported safe Sprig helper: (tuple 1 2 3 4 5 6 7 8 | mustChunk 3)`.
    #[test]
    fn a_group_in_first_position_is_a_value() {
        assert_eq!(
            render(
                r#"{{ range (tuple 1 2 3 4 5 6 7 8 | mustChunk 3) }}{{ . | join "-" }}|{{ end }}"#
            ),
            "1-2-3|4-5-6|7-8|"
        );
        assert_eq!(render(r#"{{ (upper "a") }}"#), "A");
    }

    /// Go 1.27.1 `text/template`, same templates, same data:
    /// `{{ range (list 1 2) }}` → `1;2;`, `{{ range (tuple 1 2 3 4 5 | chunk 3) }}`
    /// → `1,2,3|4,5|`, `{{ with (dict "a" 1) }}{{ .a }}{{ end }}` → `1`, and
    /// `{{ range $i, $v := (list 10 20) }}` → `0=10;1=20;`.
    #[test]
    fn a_group_is_a_value_in_every_expression_position() {
        assert_eq!(render(r"{{ range (list 1 2) }}{{ . }};{{ end }}"), "1;2;");
        assert_eq!(
            render(r#"{{ range (tuple 1 2 3 4 5 | chunk 3) }}{{ join "," . }}|{{ end }}"#),
            "1,2,3|4,5|"
        );
        assert_eq!(render(r#"{{ with (dict "a" 1) }}{{ .a }}{{ end }}"#), "1");
        assert_eq!(
            render(r"{{ range $i, $v := (list 10 20) }}{{ $i }}={{ $v }};{{ end }}"),
            "0=10;1=20;"
        );
        // A field read off a group is the same value reached the other way round.
        assert_eq!(render(r#"{{ (dict "a" 1).a }}"#), "1");
    }

    /// Two groups in one action are two values, which is not a call, so the
    /// action is refused. Go 1.27.1 reports `can't give argument to non-function
    /// upper "a"` for the first and `unexpected "(" in operand` for the second,
    /// where a token that is not one whole group must not be read as one: the
    /// body of `(a)(b)` would otherwise be `a)(b`.
    #[test]
    fn two_groups_in_one_action_are_not_a_group() {
        for source in [
            r#"{{ (upper "a") (upper "b") }}"#,
            r#"{{ (upper "a")(upper "b") }}"#,
            r"{{ range (list 1 2) (list 3) }}{{ end }}",
            r"{{ (1 2) }}",
        ] {
            assert!(
                Template::parse(source)
                    .map(|template| template.with_funcs(sprig_functions()))
                    .and_then(|template| template.render_typed(&BTreeMap::new()))
                    .is_err(),
                "{source} should not render"
            );
        }
    }
}
