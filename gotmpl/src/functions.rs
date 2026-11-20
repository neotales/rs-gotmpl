//! The functions a caller registers for a template to call.

use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use crate::{Error, Value};

/// A function a template can call, as the map stores it.
///
/// The closure takes the evaluated arguments in source order, with a piped value
/// appended as the last argument, and returns the value the expression
/// produces. It must be `Fn` rather than `FnMut` because one parsed template can
/// be rendered concurrently, and `Send + Sync` for the same reason.
pub type Function = Arc<dyn Fn(&[Value]) -> Result<Value, Error> + Send + Sync + 'static>;

/// The functions a template may call, supplied by the caller.
///
/// The engine has no built-in helper library. `len` and `index` are part of Go's
/// template language and stay in the engine; everything else — a locale-aware
/// formatter, a Sprig helper set, one project's own filter — is registered here
/// by the caller. That direction is deliberate and matches Go, where
/// `text/template` knows nothing about Sprig and Sprig is loaded into a
/// `template.FuncMap` by whoever wants it.
///
/// A name that is not registered is [`Error::UnknownFunction`], never a silently
/// empty value: a template that calls a function nobody supplied is a mistake
/// worth reporting.
///
/// ```
/// use neotales_gotmpl::{FuncMap, Template, Value};
///
/// let functions = FuncMap::new().with("shout", |arguments: &[Value]| {
///     let Some(text) = arguments.first().and_then(Value::as_str) else {
///         return Err(neotales_gotmpl::Error::Function {
///             name: "shout".to_owned(),
///             message: "shout needs a string".to_owned(),
///         });
///     };
///     Ok(Value::String(text.to_uppercase()))
/// });
///
/// let rendered = Template::parse("{{ shout .name }}")
///     .expect("parses")
///     .with_funcs(functions)
///     .render(&[("name".to_owned(), "acme".to_owned())].into_iter().collect())
///     .expect("renders");
/// assert_eq!(rendered, "ACME");
/// ```
#[derive(Clone, Default)]
pub struct FuncMap {
    functions: BTreeMap<String, Function>,
}

impl FuncMap {
    /// Creates an empty map.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers `function` under `name`, replacing any function already there.
    ///
    /// The name is matched exactly and is case-sensitive, as a Go template's
    /// function name is.
    pub fn insert(
        &mut self,
        name: impl Into<String>,
        function: impl Fn(&[Value]) -> Result<Value, Error> + Send + Sync + 'static,
    ) {
        self.functions.insert(name.into(), Arc::new(function));
    }

    /// Returns this map with `function` registered under `name`.
    ///
    /// The consuming form exists so a map can be built in one expression:
    /// `FuncMap::new().with("upper", upper).with("lower", lower)`.
    #[must_use]
    pub fn with(
        mut self,
        name: impl Into<String>,
        function: impl Fn(&[Value]) -> Result<Value, Error> + Send + Sync + 'static,
    ) -> Self {
        self.insert(name, function);
        self
    }

    /// Returns the function registered under `name`, if any.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&Function> {
        self.functions.get(name)
    }

    /// Reports whether a function is registered under `name`.
    #[must_use]
    pub fn contains(&self, name: &str) -> bool {
        self.functions.contains_key(name)
    }

    /// Returns how many functions are registered.
    #[must_use]
    pub fn len(&self) -> usize {
        self.functions.len()
    }

    /// Reports whether no function is registered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.functions.is_empty()
    }

    /// Returns the registered names, in sorted order.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.functions.keys().map(String::as_str)
    }
}

impl fmt::Debug for FuncMap {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        // The closures have no useful debug form, and their addresses would make
        // the output differ between two runs of the same program. The names are
        // what a reader wants to know anyway.
        formatter
            .debug_struct("FuncMap")
            .field("names", &self.names().collect::<Vec<_>>())
            .finish()
    }
}
