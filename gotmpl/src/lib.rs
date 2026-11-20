//! Pure, intentionally small Go `text/template` compatibility primitives.
//!
//! The engine parses and renders the Go template language and nothing else. The
//! functions a template can call beyond the language's own `len` and `index` are
//! supplied by the caller through [`FuncMap`], which is the same division Go
//! itself makes: `text/template` has no helper library, and a helper set such as
//! Sprig is loaded into it by whoever wants one.
#![doc = include_str!("../README.md")]

use std::{collections::BTreeMap, fmt, sync::Arc};

mod functions;
mod value;

pub use functions::{FuncMap, Function};
pub use value::Value;

/// Bounds applied while parsing a template source.
///
/// The defaults are finite so an untrusted source cannot cause unbounded
/// allocation or parser recursion. Limits are measured in UTF-8 bytes for
/// source length and in control-structure levels for nesting.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ParseOptions {
    /// Maximum number of UTF-8 bytes accepted as template source.
    pub max_source_length: usize,
    /// Maximum number of nested control structures accepted by the parser.
    pub max_nesting_depth: usize,
}

impl Default for ParseOptions {
    fn default() -> Self {
        Self {
            max_source_length: 1_048_576,
            max_nesting_depth: 100,
        }
    }
}

impl ParseOptions {
    /// Creates the default safe parse limits.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns these options with a different source-length limit.
    #[must_use]
    pub const fn with_max_source_length(mut self, value: usize) -> Self {
        self.max_source_length = value;
        self
    }

    /// Returns these options with a different nesting limit.
    #[must_use]
    pub const fn with_max_nesting_depth(mut self, value: usize) -> Self {
        self.max_nesting_depth = value;
        self
    }
}

/// Bounds and cooperative cancellation applied during rendering.
///
/// Rendering state is local to each call, so a parsed [`Template`] can be
/// rendered concurrently with independent options and cancellation callbacks.
#[derive(Clone, Copy)]
pub struct RenderOptions<'a> {
    /// Maximum number of UTF-8 bytes written to the final output.
    pub max_output_length: usize,
    /// Maximum number of range iterations in one render.
    pub max_iterations: usize,
    /// Maximum number of safe helper calls in one render.
    pub max_function_calls: usize,
    /// Maximum number of named-template expansions in one render.
    pub max_template_expansions: usize,
    /// Maximum number of nested AST/value rendering levels.
    pub max_nesting_depth: usize,
    /// Maximum number of AST/value evaluations in one render.
    pub max_node_evaluations: usize,
    /// Optional callback checked at parse/render evaluation boundaries.
    pub is_cancelled: Option<&'a dyn Fn() -> bool>,
    /// What a field access that names nothing produces.
    ///
    /// This mirrors the reference's `TemplateMissingValueBehavior`, and the
    /// default is its default: stop with a diagnostic.
    pub missing_value: MissingValueBehavior,
}

/// What a field access that names nothing produces.
///
/// The two answers are both correct for different authors, which is why the
/// reference makes it a choice rather than a fixed behavior.
///
/// `Error` is the default, and it is right when a template is a configuration
/// file: a typo in `{{ .env.DATABSE_URL }}` should stop the render rather than
/// quietly become an empty string that a container is then started with.
///
/// `Null` is what Go's own `text/template` does, and it is what
/// `{{ .env.OPTIONAL | default "localhost" }}` needs. Under `Error` that idiom
/// cannot be written at all, because the failure happens at the field access
/// before `default` ever sees a value — so a Compose file that wants the
/// fallback for an unset variable has no way to ask for one.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum MissingValueBehavior {
    /// Stop rendering with a missing-value diagnostic.
    #[default]
    Error,
    /// Evaluate the missing value as Go's `nil`, so a helper can supply a
    /// default for it.
    ///
    /// A null that reaches the output without passing through a helper renders
    /// as Go's `<no value>`, which is Go's own behavior and does not validate
    /// as a value. The trade is deliberate: the opt-in is for templates that
    /// supply their own fallback, and the default is what names a typo.
    Null,
}

impl Default for RenderOptions<'_> {
    fn default() -> Self {
        Self {
            max_output_length: 4_194_304,
            max_iterations: 100_000,
            max_function_calls: 100_000,
            max_template_expansions: 100_000,
            max_nesting_depth: 100,
            max_node_evaluations: 1_000_000,
            is_cancelled: None,
            missing_value: MissingValueBehavior::Error,
        }
    }
}

impl fmt::Debug for RenderOptions<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RenderOptions")
            .field("max_output_length", &self.max_output_length)
            .field("max_iterations", &self.max_iterations)
            .field("max_function_calls", &self.max_function_calls)
            .field("max_template_expansions", &self.max_template_expansions)
            .field("max_nesting_depth", &self.max_nesting_depth)
            .field("max_node_evaluations", &self.max_node_evaluations)
            .field("is_cancelled", &self.is_cancelled.is_some())
            .field("missing_value", &self.missing_value)
            .finish()
    }
}

impl<'a> RenderOptions<'a> {
    /// Creates the default safe render limits.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns these options with a different output limit.
    #[must_use]
    pub const fn with_max_output_length(mut self, value: usize) -> Self {
        self.max_output_length = value;
        self
    }

    /// Returns these options with a different range-iteration limit.
    #[must_use]
    pub const fn with_max_iterations(mut self, value: usize) -> Self {
        self.max_iterations = value;
        self
    }

    /// Returns these options with a different range-iteration limit.
    #[must_use]
    pub const fn with_max_range_iterations(self, value: usize) -> Self {
        self.with_max_iterations(value)
    }

    /// Returns these options with a different function-call limit.
    #[must_use]
    pub const fn with_max_function_calls(mut self, value: usize) -> Self {
        self.max_function_calls = value;
        self
    }

    /// Returns these options with a different named-template expansion limit.
    #[must_use]
    pub const fn with_max_template_expansions(mut self, value: usize) -> Self {
        self.max_template_expansions = value;
        self
    }

    /// Returns these options with a different named-template expansion limit.
    #[must_use]
    pub const fn with_max_named_template_expansions(self, value: usize) -> Self {
        self.with_max_template_expansions(value)
    }

    /// Returns these options with a different AST/value nesting limit.
    #[must_use]
    pub const fn with_max_nesting_depth(mut self, value: usize) -> Self {
        self.max_nesting_depth = value;
        self
    }

    /// Returns these options with a different total evaluation limit.
    #[must_use]
    pub const fn with_max_node_evaluations(mut self, value: usize) -> Self {
        self.max_node_evaluations = value;
        self
    }

    /// Returns these options with a cooperative cancellation callback.
    #[must_use]
    pub const fn with_cancellation(mut self, callback: &'a dyn Fn() -> bool) -> Self {
        self.is_cancelled = Some(callback);
        self
    }
}

/// Backwards-compatible descriptive aliases for callers that prefer the
/// longer option names used by other implementations of this engine.
///
/// All four name the same two types. The options are the only place this crate
/// takes a caller-supplied bound, and both spellings are in use, so renaming
/// either would be a breaking change for no gain.
pub type TemplateParseOptions = ParseOptions;
/// An alias for [`RenderOptions`], named for what it bounds.
pub type TemplateRenderOptions<'a> = RenderOptions<'a>;
/// An alias for [`ParseOptions`], named for what it bounds.
pub type ParseLimits = ParseOptions;
/// An alias for [`RenderOptions`], named for what it bounds.
pub type RenderLimits<'a> = RenderOptions<'a>;

#[doc(hidden)]
pub trait ParseOptionsInput {
    fn parse_options(&self) -> &ParseOptions;
}

impl ParseOptionsInput for ParseOptions {
    fn parse_options(&self) -> &ParseOptions {
        self
    }
}

impl ParseOptionsInput for &ParseOptions {
    fn parse_options(&self) -> &ParseOptions {
        self
    }
}

#[doc(hidden)]
pub trait RenderOptionsInput<'a> {
    fn render_options(&self) -> &RenderOptions<'a>;
}

impl<'a> RenderOptionsInput<'a> for RenderOptions<'a> {
    fn render_options(&self) -> &RenderOptions<'a> {
        self
    }
}

impl<'a> RenderOptionsInput<'a> for &RenderOptions<'a> {
    fn render_options(&self) -> &RenderOptions<'a> {
        self
    }
}

/// A parsed template, ready to render against data.
///
/// Parsing is separate from rendering on purpose: a template can be parsed,
/// inspected and rejected long before it is given any data, and parsing is where
/// every syntax error surfaces. Nothing here holds a reference to the data it
/// will be rendered against, so one parsed template can be rendered many times.
///
/// ```
/// use neotales_gotmpl::Template;
///
/// let template = Template::parse("hello {{ .name }}").expect("parses");
/// let values = [("name".to_owned(), "world".to_owned())]
///     .into_iter()
///     .collect();
/// assert_eq!(template.render(&values).expect("renders"), "hello world");
/// ```
#[derive(Clone, Debug)]
pub struct Template {
    parts: Vec<Part>,
    templates: BTreeMap<String, Vec<Part>>,
    functions: FuncMap,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Part {
    Text(String),
    Expression(Expression),
    Conditional {
        condition: Expression,
        truthy: Vec<Part>,
        falsy: Vec<Part>,
    },
    Range {
        values: Expression,
        declaration: Option<RangeDeclaration>,
        body: Vec<Part>,
        empty: Vec<Part>,
    },
    With {
        value: Expression,
        body: Vec<Part>,
        empty: Vec<Part>,
    },
    Assignment {
        name: String,
        value: Expression,
    },
    Template {
        name: String,
        value: Expression,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Expression {
    source: Invocation,
    pipeline: Vec<Invocation>,
}

/// The variables a `range` binds, as in `{{ range $key, $value := .mapping }}`.
///
/// Go binds two variables for a map (the key and the value) and for a list (the
/// index and the element), and one for either. Without a declaration `.` is
/// rebound to the element and nothing else is available, which is why ranging a
/// map that way loses every key.
#[derive(Clone, Debug, Eq, PartialEq)]
struct RangeDeclaration {
    /// The key for a map, or the index for a list.
    key: Option<String>,
    /// The value for a map, or the element for a list.
    value: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Invocation {
    name: Option<String>,
    arguments: Vec<Argument>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Argument {
    Path(String),
    Variable {
        name: String,
        path: String,
    },
    /// A bare token, which is typed when it is evaluated.
    ///
    /// It is kept raw rather than unwrapped so that the decision between text
    /// and a typed value happens in one place, at evaluation. Unwrapping here
    /// would turn the quoted `"42"` and the bare `42` into the same string, and a
    /// template that means the text would then be indistinguishable from one that
    /// means the number.
    Literal(String),
    /// A quoted token, which is always text.
    Text(String),
    /// A parenthesised pipeline, `( pipeline )`.
    ///
    /// Go's grammar allows an *argument* to be a whole pipeline in parentheses,
    /// which is how a composed value is passed to a helper:
    /// `{{ upper (trim " go " | repeat 2) }}`. Without this the argument
    /// tokenizes as three tokens and the helper is handed the wrong operands, so
    /// a template written that way rendered the wrong answer rather than an
    /// error. The inner text is kept and parsed as an expression on evaluation,
    /// because a pipeline cannot be evaluated at parse time.
    ///
    /// The same form is a *value* in the first position of an action or of a
    /// `range`/`if`/`with` expression, which is this variant rather than a call
    /// to a helper whose name happens to be the group.
    Grouped(String),
    /// A parenthesised pipeline with a field path applied to its result.
    ///
    /// `{{ (semver "1.3.3").Compare }}` reads a field off the value a pipeline
    /// produced. The reference supports it, and without this the action was
    /// parsed as a call to a helper whose *name* happened to be the group, so
    /// `.Compare` was passed as an argument and the call reported a missing
    /// template value for a key the caller never named.
    GroupedPath(String, String),
}

/// Everything that can go wrong while parsing or rendering a template.
///
/// Every variant carries the offending text where there is any, because a parse
/// error without the source that caused it is a parse error nobody can act on.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Error {
    /// An action opened with `{{` and was never closed.
    UnterminatedAction,
    /// A comment opened and was never closed.
    UnterminatedComment,
    /// An action contained nothing between its delimiters.
    EmptyAction,
    /// A trim marker was not preceded and followed by whitespace, which is what
    /// makes it safe to leave newlines out of the output.
    MalformedTrimMarker,
    /// A quoted string in the template was not terminated.
    InvalidQuotedString,
    /// A character literal was malformed.
    CharacterLiteral,
    /// An action used a construct this parser does not implement, with the text.
    UnsupportedAction(String),
    /// A variable had no value in the supplied data, with its name.
    MissingValue(String),
    /// A template called a name the caller's [`FuncMap`] does not hold.
    ///
    /// A name that is not registered is reported rather than rendered as an
    /// empty value, because a template calling a function nobody supplied is a
    /// mistake worth stopping for.
    UnknownFunction(String),
    /// A caller-supplied function refused the call.
    ///
    /// `name` is the name it was registered under and `message` is the
    /// function's own explanation. The engine has no helper library, so it
    /// cannot restate a helper's reason and does not try to: the message is
    /// passed through unchanged.
    ///
    /// A function that was registered through [`FuncMap::register_names`] or
    /// [`FuncMap::register_group`] usually has no name of its own at the point it
    /// fails, because one closure answers for a whole list of names. That is what
    /// [`Error::from`] a `String` produces: an unnamed function error whose
    /// message stands as it was written.
    Function {
        /// The name the function was registered under, or the empty string when
        /// the message already names it.
        name: String,
        /// What the function said went wrong.
        message: String,
    },
    /// A function group was registered with an empty name.
    ///
    /// A group is how [`FuncMap::group`] reports what was loaded, so a group with
    /// no name could not be reported. Registering it is refused rather than
    /// silently dropped.
    EmptyGroupName,
    /// A control character appeared where the grammar does not allow one, with
    /// a description of which.
    UnexpectedControl(String),
    /// A referenced variable is not defined, with its name.
    MissingVariable(String),
    /// A `define` or `template` action named a template that was never defined.
    MissingTemplate(String),
    /// The same template name was defined twice, with the name.
    DuplicateTemplate(String),
    /// A template refers to itself, directly or through another, with the chain.
    RecursiveTemplate(String),
    /// A caller-supplied limit on parse or render size was reached. The bound is
    /// in the options, so this is a caller decision rather than a parser failure.
    ResourceLimitExceeded,
    /// The caller's cancellation check returned true part-way through rendering.
    Cancelled,
}

impl fmt::Display for Error {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnterminatedAction => output.write_str("unterminated template action"),
            Self::UnterminatedComment => output.write_str("unterminated template comment"),
            Self::EmptyAction => output.write_str("empty template action"),
            Self::MalformedTrimMarker => output.write_str("malformed template trim marker"),
            Self::InvalidQuotedString => output.write_str("invalid quoted template literal"),
            Self::CharacterLiteral => output.write_str("character literals are not supported"),
            Self::UnsupportedAction(action) => {
                write!(output, "unsupported template action: {action}")
            }
            Self::MissingValue(path) => write!(output, "missing template value: .{path}"),
            Self::UnknownFunction(name) => write!(output, "unknown template function: {name}"),
            // An unnamed function error's message already says what happened, so
            // it is reported as it stands rather than behind a prefix with
            // nothing between the words.
            Self::Function { name, message } if name.is_empty() => output.write_str(message),
            Self::Function { name, message } => {
                write!(output, "template function {name} failed: {message}")
            }
            Self::EmptyGroupName => output.write_str("template function group name is empty"),
            Self::UnexpectedControl(action) => {
                write!(output, "unexpected template control action: {action}")
            }
            Self::MissingVariable(name) => write!(output, "missing template variable: ${name}"),
            Self::MissingTemplate(name) => write!(output, "missing template definition: {name}"),
            Self::DuplicateTemplate(name) => {
                write!(output, "duplicate template definition: {name}")
            }
            Self::RecursiveTemplate(name) => {
                write!(output, "recursive template definition: {name}")
            }
            Self::ResourceLimitExceeded => output.write_str("template resource limit exceeded"),
            Self::Cancelled => output.write_str("template operation cancelled"),
        }
    }
}

impl std::error::Error for Error {}

impl From<String> for Error {
    /// Reports a message from a function that did not name itself.
    ///
    /// A helper set dispatched through one closure — the shape
    /// [`FuncMap::register_names`] and [`FuncMap::register_group`] take — knows
    /// which name it is answering for inside the closure, not at the point it
    /// builds an error. This conversion is how such a closure reports a refusal
    /// without inventing a name: the message is kept as it was written, and
    /// [`fmt::Display`] prints it unchanged.
    fn from(message: String) -> Self {
        Self::Function {
            name: String::new(),
            message,
        }
    }
}

impl Template {
    /// Parses the stable literal, expression, conditional, range, and with subset.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed delimiters, unsupported actions, invalid
    /// named-template definitions, or a configured resource limit.
    pub fn parse(input: &str) -> Result<Self, Error> {
        Self::parse_with_options(input, ParseOptions::default())
    }

    /// Parses a template with explicit source and nesting limits.
    ///
    /// # Errors
    ///
    /// Returns a parse, cancellation, or resource-limit error.
    #[allow(clippy::needless_pass_by_value)]
    pub fn parse_with_options<O>(input: &str, options: O) -> Result<Self, Error>
    where
        O: ParseOptionsInput,
    {
        Self::parse_internal(input, options.parse_options(), None)
    }

    /// Parses a template with explicit limits and a cooperative cancellation
    /// callback. The callback is checked while lexing, parsing, and validating
    /// the named-template graph.
    ///
    /// # Errors
    ///
    /// Returns a parse, cancellation, or resource-limit error.
    #[allow(clippy::needless_pass_by_value)]
    pub fn parse_with_cancellation<O>(
        input: &str,
        options: O,
        is_cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, Error>
    where
        O: ParseOptionsInput,
    {
        Self::parse_internal(input, options.parse_options(), Some(is_cancelled))
    }

    /// Parses with explicit limits and a cooperative cancellation callback.
    ///
    /// # Errors
    ///
    /// Returns a parse, cancellation, or resource-limit error.
    pub fn parse_with_options_and_cancellation<O>(
        input: &str,
        options: O,
        is_cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, Error>
    where
        O: ParseOptionsInput,
    {
        Self::parse_with_cancellation(input, options, is_cancelled)
    }

    /// Alias for [`Template::parse_with_options`].
    ///
    /// # Errors
    ///
    /// Returns a parse, cancellation, or resource-limit error.
    pub fn parse_with_limits<O>(input: &str, options: O) -> Result<Self, Error>
    where
        O: ParseOptionsInput,
    {
        Self::parse_with_options(input, options)
    }

    fn parse_internal(
        input: &str,
        options: &ParseOptions,
        is_cancelled: Option<&dyn Fn() -> bool>,
    ) -> Result<Self, Error> {
        if input.len() > options.max_source_length {
            return Err(Error::ResourceLimitExceeded);
        }
        let state = ParseState {
            options: *options,
            is_cancelled,
        };
        state.check_cancelled()?;
        let lexemes = lex(input, &state)?;
        let mut position = 0;
        let mut templates = BTreeMap::new();
        let (parts, control) =
            parse_parts(&lexemes, &mut position, false, &mut templates, &state, 0)?;
        if let Some(control) = control {
            return Err(Error::UnexpectedControl(control.into()));
        }
        validate_named_template_graph(&templates, &state)?;
        Ok(Self {
            parts,
            templates,
            functions: FuncMap::new(),
        })
    }

    /// Returns this template with a caller-supplied function map attached.
    ///
    /// Calling a name the map does not hold is [`Error::UnknownFunction`].
    /// Functions live on the parsed template rather than on the render call, so
    /// one parsed template rendered concurrently uses one immutable map.
    ///
    /// ```
    /// use neotales_gotmpl::{FuncMap, Template, Value};
    ///
    /// let functions = FuncMap::new().with("twice", |arguments: &[Value]| {
    ///     let text = arguments.first().map(Value::render).unwrap_or_default();
    ///     Ok(Value::String(format!("{text}{text}")))
    /// });
    /// let template = Template::parse("{{ .name | twice }}")
    ///     .expect("parses")
    ///     .with_funcs(functions);
    /// let values = [("name".to_owned(), "go".to_owned())].into_iter().collect();
    /// assert_eq!(template.render(&values).expect("renders"), "gogo");
    /// ```
    #[must_use]
    pub fn with_funcs(mut self, functions: FuncMap) -> Self {
        self.functions = functions;
        self
    }

    /// Returns the function map this template calls.
    #[must_use]
    pub fn functions(&self) -> &FuncMap {
        &self.functions
    }

    /// Renders values without accessing host state.
    ///
    /// # Errors
    ///
    /// Returns an error when a referenced dot path is absent or a helper fails.
    pub fn render(&self, values: &BTreeMap<String, String>) -> Result<String, Error> {
        self.render_typed(&legacy_root(values))
    }

    /// Renders values with explicit limits and optional cancellation.
    ///
    /// # Errors
    ///
    /// Returns a render, cancellation, or resource-limit error.
    #[allow(clippy::needless_pass_by_value)]
    pub fn render_with_options<'a, O>(
        &self,
        values: &BTreeMap<String, String>,
        options: O,
    ) -> Result<String, Error>
    where
        O: RenderOptionsInput<'a>,
    {
        let options = options.render_options();
        if options
            .is_cancelled
            .is_some_and(|is_cancelled| is_cancelled())
        {
            return Err(Error::Cancelled);
        }
        self.render_typed_with_options(&legacy_root(values), options)
    }

    /// Alias for [`Template::render_with_options`].
    ///
    /// # Errors
    ///
    /// Returns a render, cancellation, or resource-limit error.
    pub fn render_with_limits<'a, O>(
        &self,
        values: &BTreeMap<String, String>,
        options: O,
    ) -> Result<String, Error>
    where
        O: RenderOptionsInput<'a>,
    {
        self.render_with_options(values, options)
    }

    /// Renders typed values without accessing host state.
    ///
    /// # Errors
    ///
    /// Returns an error when a referenced dot path is absent or a helper fails.
    pub fn render_typed(&self, values: &BTreeMap<String, Value>) -> Result<String, Error> {
        self.render_typed_with_options(values, RenderOptions::default())
    }

    /// Renders typed values with explicit limits and optional cancellation.
    ///
    /// # Errors
    ///
    /// Returns a render, cancellation, or resource-limit error.
    #[allow(clippy::needless_pass_by_value)]
    pub fn render_typed_with_options<'a, O>(
        &self,
        values: &BTreeMap<String, Value>,
        options: O,
    ) -> Result<String, Error>
    where
        O: RenderOptionsInput<'a>,
    {
        let mut state = RenderState::new(options.render_options(), &self.functions);
        state.check_cancelled()?;
        let root = Value::Map(values.clone());
        let mut context = Context::new(&root);
        let mut output = String::new();
        render_parts(
            &self.parts,
            &mut context,
            &self.templates,
            &mut output,
            &mut state,
        )?;
        Ok(output)
    }

    /// Alias for [`Template::render_typed_with_options`].
    ///
    /// # Errors
    ///
    /// Returns a render, cancellation, or resource-limit error.
    pub fn render_typed_with_limits<'a, O>(
        &self,
        values: &BTreeMap<String, Value>,
        options: O,
    ) -> Result<String, Error>
    where
        O: RenderOptionsInput<'a>,
    {
        self.render_typed_with_options(values, options)
    }
}

#[derive(Clone, Copy)]
struct ParseState<'a> {
    options: ParseOptions,
    is_cancelled: Option<&'a dyn Fn() -> bool>,
}

impl ParseState<'_> {
    fn check_cancelled(&self) -> Result<(), Error> {
        if self.is_cancelled.is_some_and(|is_cancelled| is_cancelled()) {
            return Err(Error::Cancelled);
        }
        Ok(())
    }

    fn check_nesting(&self, depth: usize) -> Result<(), Error> {
        self.check_cancelled()?;
        if depth > self.options.max_nesting_depth {
            return Err(Error::ResourceLimitExceeded);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Lexeme {
    Text(String),
    Action(String),
}

fn lex(input: &str, state: &ParseState<'_>) -> Result<Vec<Lexeme>, Error> {
    let mut lexemes = Vec::new();
    let mut position = 0;
    while position < input.len() {
        state.check_cancelled()?;
        let Some(relative_start) = input[position..].find("{{") else {
            push_text_lexeme(&mut lexemes, &input[position..]);
            break;
        };
        let start = position + relative_start;
        push_text_lexeme(&mut lexemes, &input[position..start]);

        let after_open = start + 2;
        let trim_left = has_left_trim_marker(&input[after_open..]);
        if trim_left {
            trim_trailing_whitespace_lexeme(&mut lexemes);
        }
        let action_start = after_open + if trim_left { 2 } else { 0 };
        if !trim_left && input[action_start..].starts_with('-') {
            action_end(&input[action_start..])?;
            return Err(Error::MalformedTrimMarker);
        }

        if input[action_start..].starts_with("/*") {
            position = comment_end(input, action_start)?;
            continue;
        }

        let end = action_end(&input[action_start..])?;
        let raw_action = &input[action_start..action_start + end];
        let trim_right = has_right_trim_suffix(raw_action);
        if raw_action.ends_with('-') && !trim_right {
            return Err(Error::MalformedTrimMarker);
        }
        let action_end = raw_action.len() - if trim_right { 2 } else { 0 };
        let action = trim_ascii(&raw_action[..action_end]);
        if action.is_empty() {
            return Err(Error::EmptyAction);
        }
        lexemes.push(Lexeme::Action(action.into()));

        position = action_start + end + 2;
        if trim_right {
            let remaining = &input[position..];
            position += remaining.len() - trim_ascii_start(remaining).len();
        }
    }
    state.check_cancelled()?;
    Ok(lexemes)
}

fn push_text_lexeme(lexemes: &mut Vec<Lexeme>, text: &str) {
    if !text.is_empty() {
        lexemes.push(Lexeme::Text(text.to_owned()));
    }
}

fn parse_parts(
    lexemes: &[Lexeme],
    position: &mut usize,
    allow_control: bool,
    templates: &mut BTreeMap<String, Vec<Part>>,
    state: &ParseState<'_>,
    depth: usize,
) -> Result<(Vec<Part>, Option<&'static str>), Error> {
    let mut parts = Vec::new();
    while let Some(lexeme) = lexemes.get(*position) {
        state.check_cancelled()?;
        *position += 1;
        match lexeme {
            Lexeme::Text(text) => parts.push(Part::Text(text.clone())),
            Lexeme::Action(action) if action.starts_with("/*") => {
                return Err(Error::UnsupportedAction(action.clone()));
            }
            Lexeme::Action(action) if matches!(action.as_str(), "else" | "end") => {
                if allow_control {
                    return Ok((parts, Some(if action == "else" { "else" } else { "end" })));
                }
                return Err(Error::UnexpectedControl(action.clone()));
            }
            Lexeme::Action(action) => {
                if let Some(condition) = control_expression(action, "if") {
                    state.check_nesting(depth + 1)?;
                    let condition = parse_expression(condition)?;
                    let (truthy, falsy) =
                        parse_block(lexemes, position, templates, state, depth + 1)?;
                    parts.push(Part::Conditional {
                        condition,
                        truthy,
                        falsy,
                    });
                } else if let Some(rest) = control_expression(action, "range") {
                    state.check_nesting(depth + 1)?;
                    let (declaration, values) = parse_range_rest(rest, action)?;
                    let values = parse_expression(values)?;
                    let (body, empty) =
                        parse_block(lexemes, position, templates, state, depth + 1)?;
                    parts.push(Part::Range {
                        body,
                        empty,
                        values,
                        declaration,
                    });
                } else if let Some(value) = control_expression(action, "with") {
                    state.check_nesting(depth + 1)?;
                    let value = parse_expression(value)?;
                    let (body, empty) =
                        parse_block(lexemes, position, templates, state, depth + 1)?;
                    parts.push(Part::With { body, empty, value });
                } else if let Some(name) = control_expression(action, "define") {
                    state.check_nesting(depth + 1)?;
                    let name = parse_definition_name(name)?;
                    let (body, empty) =
                        parse_block(lexemes, position, templates, state, depth + 1)?;
                    if !empty.is_empty() {
                        return Err(Error::UnsupportedAction(action.clone()));
                    }
                    if templates.contains_key(&name) {
                        return Err(Error::DuplicateTemplate(name));
                    }
                    templates.insert(name, body);
                } else if let Some(invocation) = control_expression(action, "template") {
                    parts.push(parse_template_invocation(invocation)?);
                } else if matches!(
                    action.as_str(),
                    "if" | "range" | "with" | "define" | "template"
                ) {
                    return Err(Error::EmptyAction);
                } else if action.starts_with('$') && action.contains(":=") {
                    let (name, value) = action
                        .split_once(":=")
                        .ok_or_else(|| Error::UnsupportedAction(action.clone()))?;
                    let name = parse_variable_name(
                        name.trim()
                            .strip_prefix('$')
                            .ok_or_else(|| Error::UnsupportedAction(action.clone()))?,
                    )?;
                    parts.push(Part::Assignment {
                        name,
                        value: parse_expression(value.trim())?,
                    });
                } else {
                    parts.push(Part::Expression(parse_expression(action)?));
                }
            }
        }
    }
    Ok((parts, None))
}

fn control_expression<'a>(action: &'a str, keyword: &str) -> Option<&'a str> {
    let expression = action.strip_prefix(keyword)?;
    let first = expression.chars().next()?;
    is_template_space(first).then(|| trim_ascii_start(expression))
}

/// Splits the part of a `range` action that follows the keyword.
///
/// Four spellings are accepted, matching Go:
///
/// - `{{ range .values }}` — no declaration, `.` is rebound per iteration;
/// - `{{ range $value := .values }}` — one variable, the value or element;
/// - `{{ range $key, $value := .mapping }}` — two variables, the key (or index)
///   and the value (or element);
/// - `{{ range $key, $value }}` and `{{ range $value }}` — the same bindings
///   without a new declaration, so they must already exist in scope.
fn parse_range_rest<'a>(
    rest: &'a str,
    action: &str,
) -> Result<(Option<RangeDeclaration>, &'a str), Error> {
    let declared = rest.trim_start().starts_with('$');
    if !declared {
        return Ok((None, rest));
    }

    // The declaration runs to the `:=`, or to the end when there is none. Only
    // the declaration may contain a comma, so splitting on the first one is
    // enough: a comma inside the expression belongs to a call such as
    // `list 1 2`, which cannot appear before `:=`.
    let (head, tail) = match rest.split_once(":=") {
        Some((head, tail)) => (head, Some(tail)),
        None => (rest, None),
    };
    let head = head.trim();
    let mut names = head.split(',').map(str::trim);
    let first = names.next().unwrap_or_default();
    let second = names.next();
    if names.next().is_some() {
        return Err(Error::UnsupportedAction(action.to_owned()));
    }

    let mut declaration = RangeDeclaration {
        key: None,
        value: None,
    };
    if let Some(second) = second {
        declaration.key = Some(parse_variable_name(strip_dollar(first, action)?)?);
        declaration.value = Some(parse_variable_name(strip_dollar(second, action)?)?);
    } else {
        declaration.value = Some(parse_variable_name(strip_dollar(first, action)?)?);
    }

    // `range $k, $v` with no `:=` is Go's assign-to-existing-scope form. It has
    // no expression to parse, and the names would have to be resolved from the
    // enclosing scope before the loop, which this subset does not model. It is
    // refused rather than silently treated as the `:=` form.
    let Some(tail) = tail else {
        return Err(Error::UnsupportedAction(action.to_owned()));
    };
    Ok((Some(declaration), tail))
}

/// Strips a leading `$`, which every Go template variable name carries.
fn strip_dollar<'a>(name: &'a str, action: &str) -> Result<&'a str, Error> {
    name.strip_prefix('$')
        .map(str::trim)
        .ok_or_else(|| Error::UnsupportedAction(action.to_owned()))
}

fn parse_block(
    lexemes: &[Lexeme],
    position: &mut usize,
    templates: &mut BTreeMap<String, Vec<Part>>,
    state: &ParseState<'_>,
    depth: usize,
) -> Result<(Vec<Part>, Vec<Part>), Error> {
    let (body, control) = parse_parts(lexemes, position, true, templates, state, depth)?;
    if control == Some("end") {
        return Ok((body, Vec::new()));
    }
    if control == Some("else") {
        let (empty, end) = parse_parts(lexemes, position, true, templates, state, depth)?;
        if end == Some("end") {
            return Ok((body, empty));
        }
    }
    Err(Error::UnexpectedControl("missing end".into()))
}

fn validate_named_template_graph(
    templates: &BTreeMap<String, Vec<Part>>,
    state: &ParseState<'_>,
) -> Result<(), Error> {
    let mut graph = BTreeMap::new();
    for (name, parts) in templates {
        state.check_cancelled()?;
        let mut calls = Vec::new();
        collect_named_template_calls(parts, &mut calls);
        calls.retain(|call| templates.contains_key(call));
        calls.sort();
        calls.dedup();
        graph.insert(name.clone(), calls);
    }

    let mut states = BTreeMap::<String, u8>::new();
    for start in graph.keys() {
        state.check_cancelled()?;
        if states.contains_key(start) {
            continue;
        }
        states.insert(start.clone(), 1);
        let mut pending = vec![(start.clone(), 0usize)];
        while let Some((name, next_target)) = pending.pop() {
            state.check_cancelled()?;
            let targets = graph
                .get(&name)
                .expect("graph nodes are inserted before traversal");
            if next_target >= targets.len() {
                states.insert(name, 2);
                continue;
            }
            pending.push((name.clone(), next_target + 1));
            let target = targets[next_target].clone();
            match states.get(&target).copied() {
                Some(1) => return Err(Error::RecursiveTemplate(target)),
                Some(2) => {}
                Some(_) => unreachable!("template graph has an invalid state"),
                None => {
                    states.insert(target.clone(), 1);
                    pending.push((target, 0));
                }
            }
        }
    }
    Ok(())
}

fn collect_named_template_calls(parts: &[Part], calls: &mut Vec<String>) {
    for part in parts {
        match part {
            Part::Template { name, .. } => calls.push(name.clone()),
            Part::Conditional { truthy, falsy, .. } => {
                collect_named_template_calls(truthy, calls);
                collect_named_template_calls(falsy, calls);
            }
            Part::Range { body, empty, .. } | Part::With { body, empty, .. } => {
                collect_named_template_calls(body, calls);
                collect_named_template_calls(empty, calls);
            }
            Part::Text(_) | Part::Expression(_) | Part::Assignment { .. } => {}
        }
    }
}

#[derive(Clone)]
struct Context {
    root: Arc<Value>,
    dot: Value,
    variables: BTreeMap<String, Value>,
}

impl Context {
    fn new(root: &Value) -> Self {
        Self {
            root: Arc::new(root.clone()),
            dot: root.clone(),
            variables: BTreeMap::new(),
        }
    }

    fn with_dot(&self, dot: &Value) -> Self {
        Self {
            root: Arc::clone(&self.root),
            dot: dot.clone(),
            variables: self.variables.clone(),
        }
    }

    fn for_named_template(dot: &Value) -> Self {
        Self {
            root: Arc::new(dot.clone()),
            dot: dot.clone(),
            variables: BTreeMap::new(),
        }
    }
}

struct RenderState<'a> {
    options: &'a RenderOptions<'a>,
    functions: &'a FuncMap,
    max_output_length: usize,
    max_iterations: usize,
    max_function_calls: usize,
    max_template_expansions: usize,
    max_nesting_depth: i64,
    max_node_evaluations: usize,
    is_cancelled: Option<&'a dyn Fn() -> bool>,
    output_length: usize,
    iterations: usize,
    function_calls: usize,
    template_expansions: usize,
    node_evaluations: usize,
    nesting_depth: i64,
    active_templates: Vec<String>,
}

impl<'a> RenderState<'a> {
    fn new(options: &'a RenderOptions<'a>, functions: &'a FuncMap) -> Self {
        Self {
            options,
            functions,
            max_output_length: options.max_output_length,
            max_iterations: options.max_iterations,
            max_function_calls: options.max_function_calls,
            max_template_expansions: options.max_template_expansions,
            max_nesting_depth: i64::try_from(options.max_nesting_depth).unwrap_or(i64::MAX),
            max_node_evaluations: options.max_node_evaluations,
            is_cancelled: options.is_cancelled,
            output_length: 0,
            iterations: 0,
            function_calls: 0,
            template_expansions: 0,
            node_evaluations: 0,
            nesting_depth: -1,
            active_templates: Vec::new(),
        }
    }

    fn check_cancelled(&self) -> Result<(), Error> {
        if self.is_cancelled.is_some_and(|is_cancelled| is_cancelled()) {
            return Err(Error::Cancelled);
        }
        Ok(())
    }

    fn observe_node(&mut self) -> Result<(), Error> {
        self.check_cancelled()?;
        if self.node_evaluations >= self.max_node_evaluations {
            return Err(Error::ResourceLimitExceeded);
        }
        self.node_evaluations += 1;
        Ok(())
    }

    fn observe_value(&mut self) -> Result<(), Error> {
        self.observe_node()
    }

    fn enter_depth(&mut self) -> Result<(), Error> {
        self.check_cancelled()?;
        self.nesting_depth = self.nesting_depth.saturating_add(1);
        if self.nesting_depth > self.max_nesting_depth {
            self.nesting_depth -= 1;
            return Err(Error::ResourceLimitExceeded);
        }
        Ok(())
    }

    fn exit_depth(&mut self) {
        self.nesting_depth = self.nesting_depth.saturating_sub(1);
    }

    fn count_iteration(&mut self) -> Result<(), Error> {
        self.check_cancelled()?;
        if self.iterations >= self.max_iterations {
            return Err(Error::ResourceLimitExceeded);
        }
        self.iterations += 1;
        Ok(())
    }

    fn count_function_call(&mut self) -> Result<(), Error> {
        self.check_cancelled()?;
        if self.function_calls >= self.max_function_calls {
            return Err(Error::ResourceLimitExceeded);
        }
        self.function_calls += 1;
        Ok(())
    }

    fn push_template(&mut self, name: &str) -> Result<(), Error> {
        self.check_cancelled()?;
        if self.active_templates.iter().any(|active| active == name) {
            return Err(Error::RecursiveTemplate(name.into()));
        }
        if self.template_expansions >= self.max_template_expansions {
            return Err(Error::ResourceLimitExceeded);
        }
        self.template_expansions += 1;
        self.active_templates.push(name.into());
        Ok(())
    }

    fn pop_template(&mut self) {
        self.active_templates.pop();
    }

    fn append(&mut self, output: &mut String, value: &str, temporary: bool) -> Result<(), Error> {
        self.check_cancelled()?;
        let base = if temporary {
            self.output_length
                .checked_add(output.len())
                .ok_or(Error::ResourceLimitExceeded)?
        } else {
            self.output_length
        };
        let new_length = base
            .checked_add(value.len())
            .ok_or(Error::ResourceLimitExceeded)?;
        if new_length > self.max_output_length {
            return Err(Error::ResourceLimitExceeded);
        }
        output.push_str(value);
        if !temporary {
            self.output_length = new_length;
        }
        Ok(())
    }

    fn render_value(
        &mut self,
        value: &Value,
        output: &mut String,
        temporary: bool,
    ) -> Result<(), Error> {
        let mut pending = vec![ValueWork::Value(value)];
        while let Some(work) = pending.pop() {
            self.check_cancelled()?;
            match work {
                ValueWork::Value(value) => {
                    self.observe_value()?;
                    match value {
                        Value::String(value) => self.append(output, value, temporary)?,
                        Value::Bool(value) => {
                            self.append(output, if *value { "true" } else { "false" }, temporary)?;
                        }
                        // A number and a null are literals, never quoted
                        // strings, so `quote` leaves them alone the way Go
                        // does. The spelling comes from the shared value
                        // renderer so a number is formatted identically
                        // wherever it is printed.
                        Value::Number(_) | Value::Float(_) | Value::Null => {
                            self.append(output, &value.render(), false)?;
                        }
                        // A method is rendered as its own name if it is ever
                        // printed rather than called, which is the best that can
                        // be said about a value that is not text.
                        Value::Method { helper, .. } => {
                            self.append(output, helper, false)?;
                        }
                        // An object renders as its display string, which is what
                        // makes `semver` usable as a value in its own right.
                        Value::Object { display, .. } => {
                            self.append(output, display, false)?;
                        }
                        Value::List(values) => {
                            if values.len()
                                > self
                                    .max_node_evaluations
                                    .saturating_sub(self.node_evaluations)
                            {
                                return Err(Error::ResourceLimitExceeded);
                            }
                            self.enter_depth()?;
                            self.append(output, "[", temporary)?;
                            pending.push(ValueWork::Exit);
                            pending.push(ValueWork::Text("]"));
                            for (index, value) in values.iter().enumerate().rev() {
                                pending.push(ValueWork::Value(value));
                                if index > 0 {
                                    pending.push(ValueWork::Text(" "));
                                }
                            }
                        }
                        Value::Map(values) => {
                            if values.len()
                                > self
                                    .max_node_evaluations
                                    .saturating_sub(self.node_evaluations)
                            {
                                return Err(Error::ResourceLimitExceeded);
                            }
                            self.enter_depth()?;
                            self.append(output, "map[", temporary)?;
                            pending.push(ValueWork::Exit);
                            pending.push(ValueWork::Text("]"));
                            for (index, (key, value)) in values.iter().enumerate().rev() {
                                pending.push(ValueWork::Value(value));
                                pending.push(ValueWork::Text(":"));
                                pending.push(ValueWork::Text(key.as_str()));
                                if index > 0 {
                                    pending.push(ValueWork::Text(" "));
                                }
                            }
                        }
                    }
                }
                ValueWork::Text(value) => self.append(output, value, temporary)?,
                ValueWork::Exit => self.exit_depth(),
            }
        }
        Ok(())
    }
}

enum ValueWork<'a> {
    Value(&'a Value),
    Text(&'a str),
    Exit,
}

fn render_parts(
    parts: &[Part],
    context: &mut Context,
    templates: &BTreeMap<String, Vec<Part>>,
    output: &mut String,
    state: &mut RenderState<'_>,
) -> Result<(), Error> {
    state.enter_depth()?;
    let result = render_parts_inner(parts, context, templates, output, state);
    state.exit_depth();
    result
}

fn render_parts_inner(
    parts: &[Part],
    context: &mut Context,
    templates: &BTreeMap<String, Vec<Part>>,
    output: &mut String,
    state: &mut RenderState<'_>,
) -> Result<(), Error> {
    for part in parts {
        state.observe_node()?;
        match part {
            Part::Text(value) => state.append(output, value, false)?,
            Part::Expression(expression) => {
                let value = expression.evaluate(context, state)?;
                state.render_value(&value, output, false)?;
            }
            Part::Assignment { name, value } => {
                let value = value.evaluate(context, state)?;
                context.variables.insert(name.clone(), value);
            }
            Part::Template { name, value } => {
                let value = value.evaluate(context, state)?;
                let template_parts = templates
                    .get(name)
                    .ok_or_else(|| Error::MissingTemplate(name.clone()))?;
                state.push_template(name)?;
                let result = {
                    let mut called = Context::for_named_template(&value);
                    render_parts(template_parts, &mut called, templates, output, state)
                };
                state.pop_template();
                result?;
            }
            Part::Conditional {
                condition,
                truthy,
                falsy,
            } => {
                let branch = if is_truthy(&condition.evaluate(context, state)?) {
                    truthy
                } else {
                    falsy
                };
                let mut branch_context = context.clone();
                render_parts(branch, &mut branch_context, templates, output, state)?;
            }
            Part::Range {
                values,
                declaration,
                body,
                empty,
            } => match values.evaluate(context, state)? {
                Value::List(values) => {
                    if values.is_empty() {
                        let mut empty_context = context.clone();
                        render_parts(empty, &mut empty_context, templates, output, state)?;
                    } else {
                        for (index, value) in values.into_iter().enumerate() {
                            state.count_iteration()?;
                            let mut body_context = range_context(
                                context,
                                declaration.as_ref(),
                                Value::String(index.to_string()),
                                &value,
                            );
                            render_parts(body, &mut body_context, templates, output, state)?;
                        }
                    }
                }
                Value::Map(values) => {
                    if values.is_empty() {
                        let mut empty_context = context.clone();
                        render_parts(empty, &mut empty_context, templates, output, state)?;
                    } else {
                        for (key, value) in values {
                            state.count_iteration()?;
                            let mut body_context = range_context(
                                context,
                                declaration.as_ref(),
                                Value::String(key),
                                &value,
                            );
                            render_parts(body, &mut body_context, templates, output, state)?;
                        }
                    }
                }
                _ => {
                    let mut empty_context = context.clone();
                    render_parts(empty, &mut empty_context, templates, output, state)?;
                }
            },
            Part::With { value, body, empty } => {
                let value = value.evaluate(context, state)?;
                if is_truthy(&value) {
                    let mut body_context = context.with_dot(&value);
                    render_parts(body, &mut body_context, templates, output, state)?;
                } else {
                    let mut empty_context = context.clone();
                    render_parts(empty, &mut empty_context, templates, output, state)?;
                }
            }
        }
    }
    Ok(())
}

/// Builds the context one `range` iteration renders with.
///
/// `.` is always rebound to the element, as Go does. When the action declared
/// variables they are bound on top: the first is the map key or the list index,
/// the second is the value or the element. A one-variable declaration binds the
/// value, which is what makes `{{ range $name := .services }}` work over a map
/// — the form that was previously rejected outright, losing every key.
fn range_context(
    context: &Context,
    declaration: Option<&RangeDeclaration>,
    key: Value,
    value: &Value,
) -> Context {
    let mut scoped = context.with_dot(value);
    let Some(declaration) = declaration else {
        return scoped;
    };
    if let Some(name) = &declaration.key {
        scoped.variables.insert(name.clone(), key);
    }
    if let Some(name) = &declaration.value {
        scoped.variables.insert(name.clone(), value.clone());
    }
    scoped
}

/// Reports whether a value is truthy, following Go's rules for the value kinds
/// a JSON or YAML values file can carry.
///
/// Go treats the zero number and a nil interface as false, and everything else
/// by emptiness. A number at zero is therefore false, which is what makes
/// `{{ if .replicas }}` mean "a nonzero replica count" as a Compose author
/// expects.
fn is_truthy(value: &Value) -> bool {
    match value {
        Value::String(value) => !value.is_empty(),
        Value::Bool(value) => *value,
        // A float is truthy by the same rule as an integer: a replica count of
        // `0.0` means the same as `0`.
        Value::Number(value) | Value::Float(value) => *value != 0.0,
        Value::Null => false,
        Value::List(values) => !values.is_empty(),
        Value::Map(values) => !values.is_empty(),
        // An object and a method are both truthy: each is something to reach
        // into or call, so a template testing whether one is present must not be
        // told the value is empty.
        Value::Object { .. } | Value::Method { .. } => true,
    }
}

/// Resolves Go's `len` and `index` builtins.
///
/// Returns `Ok(None)` for a name that is not a builtin, so the caller's
/// [`FuncMap`] is tried next. `len` and `index` are the only two the language
/// provides, and they are here because they belong to the template language:
/// `{{ len .Values }}` and `{{ index .Services 0 }}` are Go template code that
/// happens to work in a Compose file, not a helper library's.
///
/// `len` counts **bytes** for a string, which is what Go's `len` does on a
/// string and what the reference does too: `TemplateRenderer.cs::Length` returns
/// `TemplateValue.Number(Encoding.UTF8.GetByteCount(text))`. A character count is
/// the plausible reading and it is the wrong one — Go 1.27.1 `text/template`
/// renders `{{ len "café" }}` as `5` and `{{ len "日本語" }}` as `9`, and
/// compiling the reference's own method body against .NET 11 gives the same 5
/// and 9 against character counts of 4 and 3. A template sizing something with
/// `len` is out by the encoding width of every non-ASCII name.
///
/// A number, a boolean, and null have no length in Go, and an error is reported
/// rather than a zero, because a template that silently measured a number as
/// zero would produce a plausible wrong answer.
fn builtin(name: &str, arguments: &[Value]) -> Result<Option<Value>, Error> {
    match name {
        "len" => {
            let Some(value) = arguments.first() else {
                return Err(Error::UnsupportedAction(
                    "len requires a string, list, or map".into(),
                ));
            };
            if arguments.len() > 1 {
                return Err(Error::UnsupportedAction(
                    "len requires exactly one argument".into(),
                ));
            }
            // Every one of these is a count of bytes or of elements, so it is
            // never anywhere near the point where an `f64` would stop
            // representing it exactly.
            #[allow(clippy::cast_precision_loss)]
            match value {
                // `str::len` is the UTF-8 byte length, which is what Go's `len`
                // and the reference's `GetByteCount` both return.
                Value::String(text) => Ok(Some(Value::Number(text.len() as f64))),
                Value::List(values) => Ok(Some(Value::Number(values.len() as f64))),
                Value::Map(values)
                | Value::Object {
                    members: values, ..
                } => Ok(Some(Value::Number(values.len() as f64))),
                other => Err(Error::UnsupportedAction(format!(
                    "len has no length for {}",
                    other.render()
                ))),
            }
        }
        "index" => {
            if arguments.is_empty() {
                return Err(Error::UnsupportedAction("index requires a value".into()));
            }
            Ok(Some(index(&arguments[0], &arguments[1..])?))
        }
        _ => Ok(None),
    }
}

/// Go's `index`: one or more keys walked into a list, a string, or a map.
///
/// Indexing a **string** yields the byte at that position, not a character.
/// The reference's own contract test asserts it,
/// `GoTemplate/test/TemplateContractTests.cs:14-22`:
///
/// ```csharp
/// Assert.Equal(
///     "2|two|neo|97",
///     Template.Parse("{{ len .items }}|{{ index .items 1 }}|{{ index .map \"name\" }}|{{ index \"abc\" 0 }}")
///         .Render(root));
/// ```
///
/// and its `Index` reaches the same answer through
/// `Encoding.UTF8.GetBytes(text)` (`TemplateRenderer.cs:339-344`). Go 1.27.1
/// agrees on every input measured: `{{ index "abc" 0 }}` → `97`,
/// `{{ index "abc" 1 }}` → `98`, `{{ index "café" 3 }}` → `195`.
///
/// A missing key or an out-of-range position is an error. Go's `index` returns
/// the zero value in that case, and a zero value here would be indistinguishable
/// from a key that is genuinely present and empty — which is the difference
/// between a template that works and one that silently produces the wrong
/// configuration.
fn index(value: &Value, keys: &[Value]) -> Result<Value, Error> {
    let mut current = value.clone();
    for key in keys {
        current = match &current {
            Value::List(values) => {
                let position = position(key, values.len())?;
                values[position].clone()
            }
            // A byte, as a number. A position in the middle of a multi-byte
            // character is not an error in either target: Go returns the
            // continuation byte and the reference indexes the encoded array.
            Value::String(text) => {
                let bytes = text.as_bytes();
                let position = position(key, bytes.len())?;
                Value::Number(f64::from(bytes[position]))
            }
            Value::Map(values) => values.get(&key.render()).cloned().ok_or_else(|| {
                Error::UnsupportedAction(format!("index has no key {:?}", key.render()))
            })?,
            other => {
                return Err(Error::UnsupportedAction(format!(
                    "cannot index {}",
                    other.render()
                )));
            }
        };
    }
    Ok(current)
}

/// The position an `index` names, which has to be an integral, non-negative
/// number inside the thing being indexed.
///
/// Both targets refuse the three ways this used to be accepted. Go 1.27.1:
///
/// ```text
/// {{ index .l -1 }}     error calling index: index out of range: -1
/// {{ index .l 1.5 }}   error calling index: cannot index slice/array with type float64
/// {{ index .l "a" }}   error calling index: cannot index slice/array with type string
/// ```
///
/// and the reference's `IntegerIndex` (`TemplateRenderer.cs:352-363`) requires
/// `TryGetNumber`, `decimal.Truncate(number) == number`, and `number >= 0`.
///
/// A negative position counted from the end, and a fractional one truncated
/// towards zero, were Python answers in a Go template: both are places where a
/// template silently reads the wrong element instead of stopping.
///
/// A float that happens to be whole is accepted, which is the reference's
/// answer rather than Go's: `indexArg` refuses a `float64` key by type, while
/// the reference's single `decimal` has no way to tell `1` from `1.0`.
fn position(key: &Value, length: usize) -> Result<usize, Error> {
    let value = match key {
        Value::Number(value) | Value::Float(value) => *value,
        other => {
            return Err(Error::UnsupportedAction(format!(
                "index position must be a number, not {}",
                other.render()
            )));
        }
    };
    if value.fract() != 0.0 {
        return Err(Error::UnsupportedAction(format!(
            "index position {value} is not a whole number"
        )));
    }
    // The value is a whole number by now, and a length is bounded by memory, so
    // the only way this can differ is a position past what an `i64` holds — which
    // the range check below reports as out of range rather than as a wrap.
    #[allow(clippy::cast_possible_truncation)]
    let position = value as i64;
    if position < 0 {
        return Err(Error::UnsupportedAction(format!(
            "index position {position} is negative, and a negative position is not counted \
             from the end"
        )));
    }
    #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
    let position = position as usize;
    if position >= length {
        return Err(Error::UnsupportedAction(format!(
            "index {position} is out of range for {length} entries"
        )));
    }
    Ok(position)
}

impl Expression {
    fn evaluate(&self, context: &Context, state: &mut RenderState<'_>) -> Result<Value, Error> {
        let mut value = self.source.evaluate(context, None, state)?;
        for stage in &self.pipeline {
            value = stage.evaluate(context, Some(value), state)?;
        }
        Ok(value)
    }
}

impl Invocation {
    fn evaluate(
        &self,
        context: &Context,
        piped_value: Option<Value>,
        state: &mut RenderState<'_>,
    ) -> Result<Value, Error> {
        state.check_cancelled()?;
        if self.name.is_none() {
            let value = self
                .arguments
                .first()
                .ok_or_else(|| Error::UnsupportedAction("empty value".into()))
                .and_then(|argument| argument.evaluate(context, state))?;
            // A value that is a method is called rather than read. A helper can
            // return an object carrying a method, and
            // `{{ (semver "1.3.3").Compare (semver "1.2.3") }}` pipes one value
            // into the other's method. Without this the method would render as
            // its own name, which is a wrong answer rather than an obvious
            // failure.
            if let Value::Method { helper, receiver } = &value {
                let Some(piped) = piped_value else {
                    return Err(Error::UnsupportedAction(format!(
                        "{helper} is a method and needs an argument"
                    )));
                };
                // Cloned out of the map so the borrow ends before the render
                // state is touched again. A `Function` is an `Arc`, so this is a
                // refcount bump rather than a copy of the closure.
                let function = Arc::clone(
                    state
                        .functions
                        .get(helper)
                        .ok_or_else(|| Error::UnknownFunction(helper.clone()))?,
                );
                let call_arguments = [(*receiver.as_ref()).clone(), piped];
                state.count_function_call()?;
                let result = function(&call_arguments);
                state.check_cancelled()?;
                return result;
            }
            return Ok(value);
        }
        let helper = self.name.as_deref().unwrap_or_default();

        let mut arguments = self
            .arguments
            .iter()
            .map(|argument| argument.evaluate(context, state))
            .collect::<Result<Vec<_>, _>>()?;
        state.count_function_call()?;

        if let Some(value) = piped_value {
            arguments.push(value);
        }

        // Go's two length-and-index builtins are part of the template language,
        // not a helper library, so they are resolved here. A template that pipes
        // into `len` or indexes with `index` is ordinary Go template code and
        // works with no function map at all.
        if let Some(result) = builtin(helper, &arguments)? {
            state.check_cancelled()?;
            return Ok(result);
        }

        // Which functions want rendered strings and which want typed values is a
        // property of the function set, not of this engine, so the caller
        // decides it inside the closure it registers. The engine hands over the
        // evaluated values unchanged.
        let function = Arc::clone(
            state
                .functions
                .get(helper)
                .ok_or_else(|| Error::UnknownFunction(helper.to_owned()))?,
        );
        let result = function(&arguments);
        state.check_cancelled()?;
        result
    }
}

impl Argument {
    fn evaluate(&self, context: &Context, state: &mut RenderState<'_>) -> Result<Value, Error> {
        state.check_cancelled()?;
        match self {
            Self::Grouped(inner) => parse_expression(inner)?.evaluate(context, state),
            Self::GroupedPath(inner, path) => {
                let value = parse_expression(inner)?.evaluate(context, state)?;
                lookup(&value, path.trim_start_matches('.'), state)
            }
            Self::Path(path) => lookup(&context.dot, path, state),
            Self::Variable { name, path } if name.is_empty() => lookup(&context.root, path, state),
            Self::Variable { name, path } => context
                .variables
                .get(name)
                .ok_or_else(|| Error::MissingVariable(name.clone()))
                .and_then(|value| lookup(value, path, state)),
            // A bare token is a typed literal, not always a string. Go's grammar
            // gives `true`, `false`, and a number their own literal forms, and a
            // function that has to inspect a value — an emptiness test, a kind
            // report, an integer conversion — cannot tell a boolean from the four
            // characters `true` if both arrive as text. Quoted tokens were already unwrapped
            // by `parse_string_literal`, so a bare `true` is distinguishable
            // from a quoted `"true"` here: the latter kept its quotes only if the
            // value really contains them, and a template that means the text
            // writes the quotes.
            Self::Literal(value) => Ok(literal_value(value)),
            Self::Text(value) => Ok(Value::String(value.clone())),
        }
    }
}

/// Strips whichever of `prefixes` the text starts with.
fn strip_any_prefix<'a>(text: &'a str, prefixes: &[&str]) -> Option<&'a str> {
    prefixes.iter().find_map(|prefix| text.strip_prefix(prefix))
}

/// Gives a bare template token its Go literal type.
///
/// A quoted token keeps its string type, because `parse_string_literal` left the
/// quotes in place for a value that did not need unwrapping; the check for a
/// leading quote is what tells `"42"` (text) from `42` (a number).
fn literal_value(token: &str) -> Value {
    match token {
        "true" => return Value::Bool(true),
        "false" => return Value::Bool(false),
        _ => {}
    }
    if let Some((number, is_float)) = parse_number_literal(token) {
        return if is_float {
            Value::Float(number)
        } else {
            Value::Number(number)
        };
    }
    Value::String(token.to_owned())
}

/// Parses a Go numeric literal, including its base prefixes.
///
/// Go accepts `0x`, `0o`, `0b`, and `0` prefixes for the non-decimal bases, a
/// leading `0` for octal, underscores as digit separators, and a decimal point
/// or exponent for a float. A token that is not one of those is text, which is
/// why a service called `alpha` stays a string.
///
/// The second element of the pair says whether the literal was *written* as a
/// float, which is not the same question as whether its value has a fractional
/// part. `4.2` and `1e3` are floats; `0x10` and `3` are integers even though both
/// have whole values. A kind-reporting function can see the difference and a
/// whole float keeps its `.0`, so the spelling has to be carried rather than
/// recovered from the value.
fn parse_number_literal(token: &str) -> Option<(f64, bool)> {
    // An underscore is a digit separator in Go, not part of the value.
    let cleaned: String = token
        .chars()
        .filter(|character| *character != '_')
        .collect();
    if cleaned.is_empty() {
        return None;
    }
    let (sign, digits) = match cleaned.strip_prefix('-') {
        Some(rest) => (-1.0, rest),
        None => (1.0, cleaned.strip_prefix('+').unwrap_or(&cleaned)),
    };
    // A base-prefixed or octal literal is an integer however it is spelled, and a
    // decimal literal is a float only when it carries a point or an exponent. The
    // question is how the literal was *written*, not whether its value happens to
    // be whole: `4.2` and `1e3` are floats, `0x10` and `3` are integers.
    // A base-prefixed literal can be a full 64-bit word, which an `f64` cannot
    // hold exactly. Go would render the shortened value, so the shortening is
    // the behavior rather than an accident.
    #[allow(clippy::cast_precision_loss)]
    let integer =
        |digits: &str, radix: u32| i64::from_str_radix(digits, radix).ok().map(|v| v as f64);
    let (magnitude, is_float) = if let Some(rest) = strip_any_prefix(digits, &["0x", "0X"]) {
        (integer(rest, 16)?, false)
    } else if let Some(rest) = strip_any_prefix(digits, &["0o", "0O"]) {
        (integer(rest, 8)?, false)
    } else if let Some(rest) = strip_any_prefix(digits, &["0b", "0B"]) {
        (integer(rest, 2)?, false)
    } else if digits.len() > 1
        && digits.starts_with('0')
        && digits[1..].bytes().all(|b| (b'0'..=b'7').contains(&b))
    {
        // A leading `0` is octal in Go only when what follows is an octal digit,
        // so `08` is a plain number rather than a malformed literal.
        (integer(&digits[1..], 8)?, false)
    } else {
        let written_as_float = digits.contains('.') || digits.contains('e') || digits.contains('E');
        (digits.parse::<f64>().ok()?, written_as_float)
    };
    Some((sign * magnitude, is_float))
}

/// Reads a dotted field path off a value.
///
/// A path that names nothing is either an error or `null`, per
/// [`RenderOptions::missing_value`]. The distinction matters at the *last*
/// segment and not before it: `{{ .env.MISSING.DATABASE }}` is a mistake even
/// under `Null`, because there is no value to walk into, while
/// `{{ .env.MISSING | default "x" }}` is a fallback asking to be honoured.
fn lookup(dot: &Value, path: &str, state: &mut RenderState<'_>) -> Result<Value, Error> {
    state.check_cancelled()?;
    if path.is_empty() {
        return Ok(dot.clone());
    }
    let mut value = dot;
    let segments = path.split('.').collect::<Vec<_>>();
    for (index, segment) in segments.iter().enumerate() {
        state.check_cancelled()?;
        // An object is reached through the same field path as a map; only its
        // rendering differs, and that is the reference's `TemplateValue.Object`
        // distinction.
        // `Null` forgives a missing value only at the *end* of the path, because
        // only there is there a value a helper can be handed. `{{ .a.MISSING }}`
        // can become nil and reach `default`; `{{ .a.MISSING.B }}` has nothing to
        // walk into and nothing to hand on, so forgiving it would turn a typo
        // into a silently empty field — the exact failure `Error` exists to
        // prevent.
        let is_leaf = index + 1 == segments.len();
        let forgive = state.options.missing_value == MissingValueBehavior::Null && is_leaf;
        let members = match value {
            Value::Map(values) => values,
            Value::Object { members, .. } => members,
            _ if forgive => return Ok(Value::Null),
            _ => return Err(Error::MissingValue(path.into())),
        };
        match members.get(*segment) {
            Some(found) => value = found,
            None if forgive => return Ok(Value::Null),
            None => return Err(Error::MissingValue(path.into())),
        }
    }
    Ok(value.clone())
}

fn legacy_root(values: &BTreeMap<String, String>) -> BTreeMap<String, Value> {
    let mut root = BTreeMap::new();
    for (path, value) in values {
        insert_legacy_path(&mut root, &path.split('.').collect::<Vec<_>>(), value);
    }
    root
}

fn insert_legacy_path(values: &mut BTreeMap<String, Value>, path: &[&str], value: &str) {
    let Some((segment, rest)) = path.split_first() else {
        return;
    };
    if rest.is_empty() {
        values.insert((*segment).into(), Value::String(value.into()));
        return;
    }
    let entry = values
        .entry((*segment).into())
        .or_insert_with(|| Value::Map(BTreeMap::new()));
    if let Value::Map(children) = entry {
        insert_legacy_path(children, rest, value);
    }
}

fn parse_expression(action: &str) -> Result<Expression, Error> {
    let mut stages = split_pipeline(action)?
        .into_iter()
        .map(|stage| parse_invocation(&stage))
        .collect::<Result<Vec<_>, _>>()?;
    let source = (!stages.is_empty())
        .then(|| stages.remove(0))
        .ok_or_else(|| Error::UnsupportedAction(action.into()))?;
    Ok(Expression {
        source,
        pipeline: stages,
    })
}

fn parse_invocation(input: &str) -> Result<Invocation, Error> {
    let tokens = tokenize_action(input)?;
    if tokens.iter().any(|token| {
        token.starts_with('\'')
            || (!token.starts_with('"') && !token.starts_with('`') && token.contains('\''))
    }) {
        return Err(Error::CharacterLiteral);
    }
    let Some(first) = tokens.first() else {
        return Err(Error::UnsupportedAction(input.into()));
    };
    // Go's grammar lets a whole pipeline stand in parentheses wherever an
    // expression is expected, and a group in the *first* position is a value
    // rather than a call: `{{ (upper "a") }}` renders `A`, and
    // `{{ range (list 1 2) }}` iterates the two elements. A trailing `.field`
    // or `$variable` reads off the group's result rather than being passed to
    // it as arguments.
    //
    // Before this the group was only recognised in an argument position, so
    // every form above was parsed as a call to a function *named* by the group
    // and failed with an unknown-function error naming it.
    if let Some(inner) = parenthesised_group(first).filter(|_| {
        tokens[1..]
            .iter()
            .all(|token| token.starts_with('.') || token.starts_with('$'))
    }) {
        let path = tokens[1..].join("");
        return Ok(Invocation {
            name: None,
            arguments: vec![if path.is_empty() {
                Argument::Grouped(inner.to_owned())
            } else {
                Argument::GroupedPath(inner.to_owned(), path)
            }],
        });
    }

    // A bare `true`, `0`, or `0x10` is a literal rather than a helper name, so
    // `{{ 0x10 }}` renders sixteen instead of looking for a helper called
    // `0x10`. Anything that parses as a Go numeric literal or is a boolean
    // keyword is a value; everything else in first position is a call.
    let first_is_value = first.starts_with('.')
        || first.starts_with('$')
        || first.starts_with('"')
        || first.starts_with('`')
        || matches!(first.as_str(), "true" | "false")
        || parse_number_literal(first).is_some();
    if first_is_value && tokens.len() != 1 {
        return Err(Error::UnsupportedAction(input.into()));
    }
    let arguments = if first_is_value {
        &tokens[..]
    } else {
        &tokens[1..]
    };
    Ok(Invocation {
        name: (!first_is_value).then(|| first.clone()),
        arguments: arguments
            .iter()
            .map(|token| parse_argument(token))
            .collect::<Result<_, _>>()?,
    })
}

/// Returns the body of a token that is one whole `( ... )` group.
///
/// The opening parenthesis has to be the one the final `)` matches. Checking
/// only the first and last character would read `(a)(b)` — one token, two
/// groups — as a single group whose body is `a)(b`, and a quoted `(` or `)`
/// inside a group would move the depth the wrong way. Both turn a parse error
/// into a different parse error, or into an action that means something else.
fn parenthesised_group(token: &str) -> Option<&str> {
    let body = token.strip_prefix('(')?.strip_suffix(')')?;
    let mut depth = 0_i32;
    let mut closes = 0_usize;
    let mut closed_at = 0_usize;
    let mut quote = None;
    let mut escaped = false;
    for (offset, character) in token.char_indices() {
        if let Some(open) = quote {
            if escaped {
                escaped = false;
            } else if open == '"' && character == '\\' {
                escaped = true;
            } else if character == open {
                quote = None;
            }
            continue;
        }
        match character {
            '"' | '`' => quote = Some(character),
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    closes += 1;
                    if closes > 1 {
                        return None;
                    }
                    closed_at = offset + character.len_utf8();
                }
            }
            _ => {}
        }
    }
    if closes != 1 || closed_at != token.len() {
        return None;
    }
    let body = body.trim();
    (!body.is_empty()).then_some(body)
}

fn parse_argument(token: &str) -> Result<Argument, Error> {
    if let Some(inner) = parenthesised_group(token) {
        return Ok(Argument::Grouped(inner.to_owned()));
    }
    if let Some(path) = token.strip_prefix('.') {
        if path.split('.').any(str::is_empty) && !path.is_empty() {
            return Err(Error::UnsupportedAction(token.into()));
        }
        return Ok(Argument::Path(path.into()));
    }
    if let Some(variable) = token.strip_prefix('$') {
        let (name, path) = variable.split_once('.').unwrap_or((variable, ""));
        return Ok(Argument::Variable {
            name: if name.is_empty() {
                String::new()
            } else {
                parse_variable_name(name)?
            },
            path: path.into(),
        });
    }
    // A quoted token is text by construction, so it is unwrapped now. A bare one
    // is kept raw and typed on evaluation, where the whole literal grammar is
    // available.
    if token.starts_with('"') || token.starts_with('`') || token.starts_with('\'') {
        return Ok(Argument::Text(parse_string_literal(token)?));
    }
    Ok(Argument::Literal(token.to_owned()))
}

fn split_pipeline(input: &str) -> Result<Vec<String>, Error> {
    let mut stages = Vec::new();
    let mut stage = String::new();
    let mut quote = None;
    let mut escaped = false;
    let mut group_depth = 0usize;
    for character in input.chars() {
        if let Some(active_quote) = quote {
            if active_quote == '`' {
                if character == '`' {
                    quote = None;
                }
            } else {
                if character == '\n' {
                    return Err(Error::InvalidQuotedString);
                }
                if escaped {
                    escaped = false;
                } else if character == '\\' {
                    escaped = true;
                } else if character == active_quote {
                    quote = None;
                }
            }
            stage.push(character);
            continue;
        }

        if group_depth > 0 {
            // A `|` inside parentheses belongs to the grouped pipeline, not to
            // the enclosing one, so the depth has to be tracked before the
            // separator below is considered.
            match character {
                '(' => group_depth += 1,
                ')' => group_depth = group_depth.saturating_sub(1),
                _ => {}
            }
            stage.push(character);
            continue;
        }

        match character {
            '"' | '`' | '\'' => {
                quote = Some(character);
                stage.push(character);
            }
            '(' => {
                group_depth += 1;
                stage.push(character);
            }
            '|' => {
                if trim_ascii(&stage).is_empty() {
                    return Err(Error::EmptyAction);
                }
                stages.push(std::mem::take(&mut stage));
            }
            _ => stage.push(character),
        }
    }
    if quote.is_some() {
        return Err(Error::UnterminatedAction);
    }
    if trim_ascii(&stage).is_empty() {
        return Err(Error::EmptyAction);
    }
    stages.push(stage);
    Ok(stages)
}

fn tokenize_action(input: &str) -> Result<Vec<String>, Error> {
    let mut tokens = Vec::new();
    let mut token = String::new();
    let mut quote = None;
    let mut escaped = false;
    // How deeply nested the parenthesis currently being accumulated is. A group
    // may contain spaces, and a space inside one is part of the group rather
    // than a separator, so the depth is what tells the two apart.
    let mut group_depth = 0usize;
    for character in trim_ascii(input).chars() {
        if let Some(active_quote) = quote {
            if active_quote == '`' {
                if character == '`' {
                    quote = None;
                }
            } else {
                if character == '\n' {
                    return Err(Error::InvalidQuotedString);
                }
                if escaped {
                    escaped = false;
                } else if character == '\\' {
                    escaped = true;
                } else if character == active_quote {
                    quote = None;
                }
            }
            token.push(character);
            continue;
        }

        if group_depth > 0 {
            // Inside a group every character is kept, including spaces and
            // further parentheses, so the group reaches `parse_argument` whole.
            match character {
                '(' => group_depth += 1,
                ')' => group_depth = group_depth.saturating_sub(1),
                _ => {}
            }
            token.push(character);
            if group_depth == 0 {
                tokens.push(std::mem::take(&mut token));
            }
            continue;
        }

        if is_template_space(character) {
            if !token.is_empty() {
                tokens.push(std::mem::take(&mut token));
            }
        } else if character.is_whitespace() {
            return Err(Error::UnsupportedAction(input.into()));
        } else {
            match character {
                '"' | '`' | '\'' => quote = Some(character),
                '(' => {
                    // A group starts here and is accumulated above.
                    if !token.is_empty() {
                        tokens.push(std::mem::take(&mut token));
                    }
                    group_depth = 1;
                    token.push(character);
                    continue;
                }
                _ => {}
            }
            token.push(character);
        }
    }
    if quote.is_some() {
        return Err(Error::UnsupportedAction(input.into()));
    }
    if group_depth > 0 {
        // An unbalanced `(` would otherwise be reported as a stray literal.
        return Err(Error::UnsupportedAction(input.into()));
    }
    if !token.is_empty() {
        tokens.push(token);
    }
    Ok(tokens)
}

fn action_end(input: &str) -> Result<usize, Error> {
    let mut quote = None;
    let mut escaped = false;
    for (index, character) in input.char_indices() {
        if let Some(active_quote) = quote {
            if active_quote == '`' {
                if character == '`' {
                    quote = None;
                }
            } else {
                if character == '\n' {
                    return Err(Error::InvalidQuotedString);
                }
                if escaped {
                    escaped = false;
                } else if character == '\\' {
                    escaped = true;
                } else if character == active_quote {
                    quote = None;
                }
            }
            continue;
        }

        match character {
            '"' | '`' | '\'' => quote = Some(character),
            '}' if input[index..].starts_with("}}") => return Ok(index),
            _ => {}
        }
    }
    Err(Error::UnterminatedAction)
}

fn comment_end(input: &str, start: usize) -> Result<usize, Error> {
    let body_start = start + 2;
    let Some(relative_end) = input[body_start..].find("*/") else {
        return Err(Error::UnterminatedComment);
    };
    let after_comment = body_start + relative_end + 2;
    let rest = &input[after_comment..];
    let trim_right = rest.len() >= 2 && has_right_trim_marker(rest) && rest[2..].starts_with("}}");
    if !trim_right && !rest.starts_with("}}") {
        return Err(Error::UnterminatedComment);
    }

    let delimiter_start = after_comment + if trim_right { 2 } else { 0 };
    let mut end = delimiter_start + 2;
    if trim_right {
        let following = &input[end..];
        end += following.len() - trim_ascii_start(following).len();
    }
    Ok(end)
}

fn has_left_trim_marker(input: &str) -> bool {
    let bytes = input.as_bytes();
    bytes.len() >= 2 && bytes[0] == b'-' && is_template_space(char::from(bytes[1]))
}

fn has_right_trim_marker(input: &str) -> bool {
    let bytes = input.as_bytes();
    bytes.len() >= 2 && is_template_space(char::from(bytes[0])) && bytes[1] == b'-'
}

fn has_right_trim_suffix(input: &str) -> bool {
    let bytes = input.as_bytes();
    bytes.len() >= 2
        && is_template_space(char::from(bytes[bytes.len() - 2]))
        && bytes[bytes.len() - 1] == b'-'
}

fn is_template_space(character: char) -> bool {
    matches!(character, ' ' | '\t' | '\r' | '\n')
}

fn trim_ascii(input: &str) -> &str {
    input.trim_matches(is_template_space)
}

fn trim_ascii_start(input: &str) -> &str {
    input.trim_start_matches(is_template_space)
}

fn parse_variable_name(input: &str) -> Result<String, Error> {
    if input.is_empty()
        || !input
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '_')
    {
        return Err(Error::UnsupportedAction(format!("${input}")));
    }
    Ok(input.into())
}

fn parse_string_literal(token: &str) -> Result<String, Error> {
    if token.starts_with('\'') {
        return Err(Error::CharacterLiteral);
    }
    if token.starts_with('"') {
        if !token.ends_with('"') || token.len() < 2 {
            return Err(Error::InvalidQuotedString);
        }
        return decode_quoted_string(&token[1..token.len() - 1]).ok_or(Error::InvalidQuotedString);
    }
    if token.starts_with('`') {
        if !token.ends_with('`') || token.len() < 2 {
            return Err(Error::UnsupportedAction(token.into()));
        }
        return Ok(token[1..token.len() - 1].into());
    }
    Ok(token.into())
}

fn decode_quoted_string(input: &str) -> Option<String> {
    let mut output = Vec::new();
    let mut characters = input.chars();
    while let Some(character) = characters.next() {
        if character != '\\' {
            if character == '\n' {
                return None;
            }
            let mut encoded = [0; 4];
            output.extend_from_slice(character.encode_utf8(&mut encoded).as_bytes());
            continue;
        }

        match characters.next()? {
            'a' => output.push(0x07),
            'b' => output.push(0x08),
            'f' => output.push(0x0c),
            'n' => output.push(b'\n'),
            'r' => output.push(b'\r'),
            't' => output.push(b'\t'),
            'v' => output.push(0x0b),
            '\\' => output.push(b'\\'),
            '"' => output.push(b'"'),
            'x' => output.push(u8::try_from(read_radix_escape(&mut characters, 2, 16)?).ok()?),
            'u' => append_unicode_escape(&mut output, &mut characters, 4)?,
            'U' => append_unicode_escape(&mut output, &mut characters, 8)?,
            first @ '0'..='7' => {
                let mut value = u32::from(first as u8 - b'0');
                for _ in 0..2 {
                    let digit = characters.next()?;
                    if !matches!(digit, '0'..='7') {
                        return None;
                    }
                    value = value * 8 + u32::from(digit as u8 - b'0');
                }
                if value > u32::from(u8::MAX) {
                    return None;
                }
                output.push(u8::try_from(value).ok()?);
            }
            _ => return None,
        }
    }
    String::from_utf8(output).ok()
}

fn read_radix_escape(
    characters: &mut std::str::Chars<'_>,
    count: usize,
    radix: u32,
) -> Option<u32> {
    let mut value = 0;
    for _ in 0..count {
        let character = characters.next()?;
        let digit = character
            .to_digit(radix)
            .filter(|_| character.is_ascii_hexdigit())?;
        value = value * radix + digit;
    }
    Some(value)
}

fn append_unicode_escape(
    output: &mut Vec<u8>,
    characters: &mut std::str::Chars<'_>,
    count: usize,
) -> Option<()> {
    let value = read_radix_escape(characters, count, 16)?;
    let character = char::from_u32(value)?;
    let mut encoded = [0; 4];
    output.extend_from_slice(character.encode_utf8(&mut encoded).as_bytes());
    Some(())
}

fn parse_definition_name(input: &str) -> Result<String, Error> {
    let tokens = tokenize_action(input)?;
    if tokens.len() != 1 {
        return Err(Error::UnsupportedAction(input.into()));
    }
    parse_string_literal(&tokens[0])
}

fn parse_template_invocation(input: &str) -> Result<Part, Error> {
    let tokens = tokenize_action(input)?;
    if tokens.len() < 2 {
        return Err(Error::UnsupportedAction(input.into()));
    }
    let name = parse_string_literal(&tokens[0])?;
    let value = parse_expression(&tokens[1..].join(" "))?;
    Ok(Part::Template { name, value })
}

fn trim_trailing_whitespace_lexeme(lexemes: &mut [Lexeme]) {
    if let Some(Lexeme::Text(value)) = lexemes.last_mut() {
        *value = value.trim_end_matches(is_template_space).to_owned();
    }
}

#[cfg(test)]
mod tests {
    use super::{Error, FuncMap, MissingValueBehavior, RenderOptions, Template, Value};
    use std::collections::BTreeMap;

    #[test]
    fn renders_dot_paths_comments_and_whitespace_trimming() {
        let template = Template::parse("hello {{- .env.NAME -}} {{/* ignored */}}!").unwrap();
        let values = BTreeMap::from([("env.NAME".into(), "acme".into())]);
        assert_eq!(template.render(&values).unwrap(), "helloacme!");
    }

    #[test]
    fn reports_missing_values_without_substitution() {
        let template = Template::parse("{{ .env.MISSING }}").unwrap();
        assert!(template.render(&BTreeMap::new()).is_err());
    }

    /// `Null` only forgives a value a helper can stand in for.
    ///
    /// `{{ .env.MISSING.DATABASE }}` has no value to walk into and no helper to
    /// receive one, so forgiving it would turn a typo into a silently empty
    /// field — the failure mode `Error` exists to prevent.
    #[test]
    fn null_does_not_forgive_walking_through_a_missing_value() {
        let template = Template::parse("{{ .env.MISSING.DATABASE }}").expect("parses");
        let lenient = RenderOptions {
            missing_value: MissingValueBehavior::Null,
            ..RenderOptions::default()
        };
        assert!(
            template
                .render_with_options(&BTreeMap::new(), lenient)
                .is_err(),
            "walking through a value that does not exist is a mistake either way"
        );
    }

    /// A name the caller's map does not hold is reported, never rendered as an
    /// empty value.
    ///
    /// This is what the parity suite relies on to mean anything: a suite where an
    /// unknown function silently rendered nothing would pass by not trying.
    #[test]
    fn an_unknown_function_is_reported_rather_than_rendered() {
        let error = Template::parse("{{ notAHelper \"x\" }}")
            .unwrap()
            .render(&BTreeMap::new())
            .expect_err("an unknown function must not render");
        assert_eq!(error, Error::UnknownFunction("notAHelper".to_owned()));
    }

    /// The language's own builtins need no function map at all.
    #[test]
    fn the_builtins_need_no_function_map() {
        let template = Template::parse(r#"{{ len "abc" }}|{{ index "abc" 0 }}"#).unwrap();
        assert_eq!(template.render(&BTreeMap::new()).unwrap(), "3|97");
    }

    /// A registered function receives the evaluated arguments, with a piped value
    /// appended as the last one.
    #[test]
    fn a_registered_function_receives_the_evaluated_arguments() {
        let functions = FuncMap::new().with("add-all", |arguments: &[Value]| {
            Ok(Value::Number(arguments.iter().map(Value::as_number).sum()))
        });
        let template = Template::parse("{{ add-all 1 2 3 }}|{{ 4 | add-all }}")
            .expect("parses")
            .with_funcs(functions);
        assert_eq!(template.render(&BTreeMap::new()).expect("renders"), "6|4");
    }

    /// A function that refuses stops the render and its own message is what the
    /// caller sees.
    #[test]
    fn a_function_that_refuses_stops_the_render() {
        let functions = FuncMap::new().with("refuse", |_: &[Value]| {
            Err(Error::Function {
                name: "refuse".to_owned(),
                message: "not today".to_owned(),
            })
        });
        let error = Template::parse("{{ refuse }}")
            .expect("parses")
            .with_funcs(functions)
            .render(&BTreeMap::new())
            .expect_err("the function refused");
        assert_eq!(
            error.to_string(),
            "template function refuse failed: not today"
        );
    }

    #[test]
    fn renders_typed_nested_paths_with_range_and_with_dot_scoping() {
        let template = Template::parse(
            "{{ range .services }}{{ with .meta }}{{ .name }}={{ .port }};{{ end }}{{ else }}empty{{ end }}",
        )
        .unwrap();
        let values = BTreeMap::from([(
            "services".into(),
            Value::List(vec![
                Value::Map(BTreeMap::from([(
                    "meta".into(),
                    Value::Map(BTreeMap::from([
                        ("name".into(), Value::String("api".into())),
                        ("port".into(), Value::String("8080".into())),
                    ])),
                )])),
                Value::Map(BTreeMap::from([(
                    "meta".into(),
                    Value::Map(BTreeMap::from([
                        ("name".into(), Value::String("web".into())),
                        ("port".into(), Value::String("3000".into())),
                    ])),
                )])),
            ]),
        )]);
        assert_eq!(
            template.render_typed(&values).unwrap(),
            "api=8080;web=3000;"
        );
    }

    #[test]
    fn preserves_outer_dot_for_empty_with_and_range() {
        let template = Template::parse(
            "{{ with .missing }}bad{{ else }}{{ .name }}{{ end }}:{{ range .items }}bad{{ else }}{{ .name }}{{ end }}",
        )
        .unwrap();
        let values = BTreeMap::from([
            ("name".into(), Value::String("acme".into())),
            ("missing".into(), Value::String(String::new())),
            ("items".into(), Value::List(Vec::new())),
        ]);
        assert_eq!(template.render_typed(&values).unwrap(), "acme:acme");
    }

    #[test]
    fn ranges_maps_in_key_order() {
        let template = Template::parse("{{ range .labels }}{{ . }};{{ end }}").unwrap();
        let values = BTreeMap::from([(
            "labels".into(),
            Value::Map(BTreeMap::from([
                ("b".into(), Value::String("second".into())),
                ("a".into(), Value::String("first".into())),
            ])),
        )]);
        assert_eq!(template.render_typed(&values).unwrap(), "first;second;");
    }

    #[test]
    fn supports_variables_and_root_scope_across_dot_changes() {
        let template = Template::parse(
            "{{ $prefix := .prefix }}{{ range .items }}{{ $prefix }}{{ . }}={{ $.suffix }};{{ end }}",
        )
        .unwrap();
        let values = BTreeMap::from([
            ("prefix".into(), Value::String("item-".into())),
            ("suffix".into(), Value::String("ok".into())),
            (
                "items".into(),
                Value::List(vec![Value::String("a".into()), Value::String("b".into())]),
            ),
        ]);
        assert_eq!(
            template.render_typed(&values).unwrap(),
            "item-a=ok;item-b=ok;"
        );
    }

    #[test]
    fn requires_spaced_trim_markers_and_trims_only_ascii_whitespace() {
        let template = Template::parse("a \t\n{{- .value -}}\r\n\tb").unwrap();
        let values = BTreeMap::from([("value".into(), "x".into())]);
        assert_eq!(template.render(&values).unwrap(), "axb");

        let unicode = Template::parse("a\u{00a0}{{- .value}}\u{00a0}b").unwrap();
        assert_eq!(unicode.render(&values).unwrap(), "a\u{00a0}x\u{00a0}b");

        assert!(Template::parse("a{{-.value}}b").is_err());
        assert!(Template::parse("a{{ .value-}}b").is_err());
    }

    #[test]
    fn validates_comment_closure_before_discarding_comments() {
        let template = Template::parse("a{{/* comment }} still */}}b").unwrap();
        assert_eq!(template.render(&BTreeMap::new()).unwrap(), "ab");

        for source in [
            "{{/* unterminated }}",
            "{{/* comment */ }}",
            "{{/* comment */",
            "{{}}",
            "{{   }}",
            "{{ | .value }}",
            "{{ .value | }}",
        ] {
            assert!(Template::parse(source).is_err(), "accepted {source:?}");
        }
    }

    #[test]
    fn rejects_character_literals_and_raw_newlines_in_quoted_strings() {
        for source in [
            "{{ 'x' }}",
            "{{ default 'x' .value }}",
            "{{ \"line\nbreak\" }}",
        ] {
            assert!(Template::parse(source).is_err(), "accepted {source:?}");
        }
    }

    #[test]
    fn renders_named_templates_with_nested_calls() {
        let template = Template::parse(
            "{{ define \"line\" }}{{ .name }}{{ end }}{{ define \"list\" }}{{ range . }}{{ template \"line\" . }};{{ end }}{{ end }}{{ template \"list\" .items }}",
        )
        .unwrap();
        let values = BTreeMap::from([(
            "items".into(),
            Value::List(vec![
                Value::Map(BTreeMap::from([(
                    "name".into(),
                    Value::String("api".into()),
                )])),
                Value::Map(BTreeMap::from([(
                    "name".into(),
                    Value::String("web".into()),
                )])),
            ]),
        )]);
        assert_eq!(template.render_typed(&values).unwrap(), "api;web;");
    }
}

#[cfg(test)]
mod builtin_tests {
    use super::{Template, Value, builtin, index};
    use std::collections::BTreeMap;

    fn list(values: &[Value]) -> Value {
        Value::List(values.to_vec())
    }

    fn number(value: f64) -> Value {
        Value::Number(value)
    }

    #[test]
    fn len_counts_the_bytes_of_a_string() {
        // Go's `len` on a string is its length in bytes, and the reference's
        // `Length` is `Encoding.UTF8.GetByteCount(text)`. Measured on both:
        //
        //   input     Go 1.27.1   .NET reference body   characters
        //   "two"          3                 3                3
        //   "café"         5                 5                4
        //   "a😀b"         6                 6                4
        //   ""             0                 0                0
        //   "åäö"          6                 6                3
        //   "日本語"        9                 9                3
        //
        // A character count is the plausible reading and it disagrees with both
        // targets on every non-ASCII input.
        for (input, expected) in [
            ("two", 3.0),
            ("café", 5.0),
            ("a😀b", 6.0),
            ("", 0.0),
            ("åäö", 6.0),
            ("日本語", 9.0),
        ] {
            assert_eq!(
                builtin("len", &[Value::String(input.to_owned())]),
                Ok(Some(number(expected))),
                "len of {input:?}"
            );
        }
        assert_eq!(
            builtin("len", &[Value::String("naïve".to_owned())]),
            Ok(Some(number(6.0)))
        );
    }

    #[test]
    fn len_counts_list_and_map_entries() {
        assert_eq!(
            builtin("len", &[list(&[number(1.0), number(2.0), number(3.0)])]),
            Ok(Some(number(3.0)))
        );
        let map = Value::Map(BTreeMap::from([
            ("a".to_owned(), number(1.0)),
            ("b".to_owned(), number(2.0)),
        ]));
        assert_eq!(builtin("len", &[map]), Ok(Some(number(2.0))));
    }

    /// Go has no length for a number, a boolean, or nil. A zero would be
    /// indistinguishable from a real length, so this is an error.
    #[test]
    fn len_refuses_a_value_that_has_no_length() {
        for value in [number(3.0), Value::Bool(true), Value::Null] {
            assert!(
                builtin("len", &[value]).is_err(),
                "len must refuse a value with no length"
            );
        }
    }

    #[test]
    fn len_needs_exactly_one_argument() {
        assert!(builtin("len", &[]).is_err());
        assert!(builtin("len", &[number(1.0), number(2.0)]).is_err());
    }

    #[test]
    fn index_walks_a_list_and_a_map() {
        let values = list(&[Value::String("a".to_owned()), Value::String("b".to_owned())]);
        assert_eq!(
            index(&values, &[number(1.0)]),
            Ok(Value::String("b".to_owned()))
        );
        let map = Value::Map(BTreeMap::from([("key".to_owned(), number(7.0))]));
        assert_eq!(
            index(&map, &[Value::String("key".to_owned())]),
            Ok(number(7.0))
        );
        // Nested: a list of maps, which is what a Compose document is.
        let nested = list(&[map]);
        assert_eq!(
            index(&nested, &[number(0.0), Value::String("key".to_owned())]),
            Ok(number(7.0))
        );
    }

    /// Indexing a string yields its **byte**, which is what the reference's own
    /// contract test asserts (`TemplateContractTests.cs:14-22` expects the `97`
    /// in `"2|two|neo|97"` for `{{ index "abc" 0 }}`) and what Go 1.27.1
    /// returns.
    #[test]
    fn indexing_a_string_yields_a_byte() {
        // Go 1.27.1: 97, 98, and 195 for the last byte of "café".
        assert_eq!(
            index(&Value::String("abc".to_owned()), &[number(0.0)]),
            Ok(number(97.0))
        );
        assert_eq!(
            index(&Value::String("abc".to_owned()), &[number(1.0)]),
            Ok(number(98.0))
        );
        assert_eq!(
            index(&Value::String("café".to_owned()), &[number(3.0)]),
            Ok(number(195.0))
        );
        // A position in the middle of a multi-byte character is a continuation
        // byte in both targets, not an error.
        assert_eq!(
            index(&Value::String("café".to_owned()), &[number(4.0)]),
            Ok(number(169.0))
        );
        // A string indexed by something that is not a number is refused, as
        // `{{ index "abc" "a" }}` is in Go: `cannot index slice/array with type
        // string`.
        assert!(
            index(
                &Value::String("abc".to_owned()),
                &[Value::String("a".to_owned())]
            )
            .is_err()
        );
    }

    /// A position is a non-negative whole number. Both targets refuse anything
    /// else, and each of the three shapes below used to be answered here.
    #[test]
    fn an_index_position_must_be_a_whole_non_negative_number() {
        let values = list(&[
            Value::String("a".to_owned()),
            Value::String("b".to_owned()),
            Value::String("c".to_owned()),
        ]);
        // Go 1.27.1: `index out of range: -1`. Counting from the end is a
        // Python answer, and it read `c` for `-1`.
        assert!(index(&values, &[number(-1.0)]).is_err());
        // Go 1.27.1: `cannot index slice/array with type float64`. Truncating
        // read `b` for `1.5`.
        assert!(index(&values, &[number(1.5)]).is_err());
        // Go 1.27.1: `cannot index slice/array with type bool`.
        assert!(index(&values, &[Value::Bool(true)]).is_err());
        // A whole float is accepted, which is the reference's answer: its
        // `IntegerIndex` has one decimal type and cannot tell `1` from `1.0`.
        assert_eq!(
            index(&values, &[Value::Float(1.0)]),
            Ok(Value::String("b".to_owned()))
        );
    }

    /// A missing key must be an error rather than Go's zero value: a zero here
    /// would look like a key that is present and empty.
    #[test]
    fn a_missing_key_or_position_is_an_error() {
        let values = list(&[Value::String("a".to_owned())]);
        assert!(index(&values, &[number(5.0)]).is_err());
        assert!(index(&values, &[number(-5.0)]).is_err());
        let map = Value::Map(BTreeMap::new());
        assert!(index(&map, &[Value::String("absent".to_owned())]).is_err());
        assert!(index(&number(1.0), &[number(0.0)]).is_err());
    }

    /// The reference's contract, run end to end through the engine.
    ///
    /// `TemplateContractTests.cs:14-22` asserts `"2|two|neo|97"` for
    /// `{{ len .items }}|{{ index .items 1 }}|{{ index .map "name" }}|{{ index "abc" 0 }}`.
    /// The last cell used to be `cannot index abc`.
    #[test]
    fn the_reference_len_and_index_contract_renders() {
        let mut root = BTreeMap::new();
        root.insert(
            "items".to_owned(),
            list(&[
                Value::String("one".to_owned()),
                Value::String("two".to_owned()),
            ]),
        );
        root.insert(
            "map".to_owned(),
            Value::Map(BTreeMap::from([(
                "name".to_owned(),
                Value::String("neo".to_owned()),
            )])),
        );
        let rendered = Template::parse(
            r#"{{ len .items }}|{{ index .items 1 }}|{{ index .map "name" }}|{{ index "abc" 0 }}"#,
        )
        .expect("parses")
        .render_typed(&root)
        .expect("renders");
        assert_eq!(rendered, "2|two|neo|97");
    }

    #[test]
    fn a_name_that_is_not_a_builtin_is_passed_on() {
        assert_eq!(builtin("upper", &[Value::String("x".to_owned())]), Ok(None));
    }

    /// The builtins have to be reachable through the template, not only as
    /// functions, or a Compose file that pipes into `len` would not render.
    #[test]
    fn the_builtins_render_through_a_template() {
        let mut root = BTreeMap::new();
        root.insert(
            "values".to_owned(),
            Value::Map(BTreeMap::from([(
                "a".to_owned(),
                Value::String("x".to_owned()),
            )])),
        );
        root.insert(
            "services".to_owned(),
            Value::List(vec![Value::String("web".to_owned())]),
        );
        let rendered = Template::parse("{{ len .values }}-{{ index .services 0 }}")
            .expect("parses")
            .render_typed(&root)
            .expect("renders");
        assert_eq!(rendered, "1-web");
    }

    /// Go 1.27.1 `text/template`, the same three templates:
    /// `{{ len "café" }}` → `5`, `{{ "café" | len }}` → `5`,
    /// `{{ len "日本語" }}` → `9`.
    #[test]
    fn len_measures_a_string_in_bytes_through_a_template() {
        for (source, expected) in [
            (r#"{{ len "café" }}"#, "5"),
            (r#"{{ "café" | len }}"#, "5"),
            (r#"{{ len "日本語" }}"#, "9"),
        ] {
            let rendered = Template::parse(source)
                .expect("parses")
                .render_typed(&BTreeMap::new())
                .expect("renders");
            assert_eq!(rendered, expected, "{source}");
        }
    }
}
