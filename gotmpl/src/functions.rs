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
/// There are three ways in, and none of them names a helper library:
///
/// - [`FuncMap::register`] adds one function.
/// - [`FuncMap::register_names`] adds a name list through one `call` closure.
/// - [`FuncMap::register_group`] does the same and records the names under a
///   group name the caller chose, so the map can report what was loaded.
///
/// A set of functions is loaded by the code that owns the set, and the engine
/// sees only names and closures. A name that is not registered is
/// [`Error::UnknownFunction`], never a silently empty value: a template that
/// calls a function nobody supplied is a mistake worth reporting.
///
/// ```
/// use neotales_gotmpl::{FuncMap, Template, Value};
///
/// let mut functions = FuncMap::new();
/// functions.register("shout", |arguments: &[Value]| {
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
    groups: BTreeMap<String, Vec<String>>,
}

impl FuncMap {
    /// Creates an empty map.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers `function` under `name`, replacing any function already there.
    ///
    /// This is the one way a function is added, whether the caller owns one
    /// function or a whole helper library. The name is matched exactly and is
    /// case-sensitive, as a Go template's function name is. Adding a second
    /// function never clears the map.
    pub fn register(
        &mut self,
        name: impl Into<String>,
        function: impl Fn(&[Value]) -> Result<Value, Error> + Send + Sync + 'static,
    ) {
        self.functions.insert(name.into(), Arc::new(function));
    }

    /// Registers every name in `names` against one `call` closure.
    ///
    /// `call` is invoked with the name of the helper being called and the
    /// arguments the engine evaluated, so a helper set is loaded by naming its
    /// functions once and dispatching on the name inside the closure — the shape
    /// a helper library's own `call(name, arguments)` already has. A second call
    /// adds more names; it does not clear the map.
    ///
    /// ```
    /// use neotales_gotmpl::{FuncMap, Template, Value};
    ///
    /// let mut functions = FuncMap::new();
    /// functions.register_names(&["shout", "whisper"], |name, arguments| {
    ///     let text = arguments.first().map(Value::render).unwrap_or_default();
    ///     Ok(Value::String(if name == "shout" {
    ///         text.to_uppercase()
    ///     } else {
    ///         text.to_lowercase()
    ///     }))
    /// });
    ///
    /// let values = [("name".to_owned(), "Acme".to_owned())].into_iter().collect();
    /// let rendered = Template::parse("{{ .name | shout }}/{{ .name | whisper }}")
    ///     .expect("parses")
    ///     .with_funcs(functions)
    ///     .render(&values)
    ///     .expect("renders");
    /// assert_eq!(rendered, "ACME/acme");
    /// ```
    pub fn register_names<C>(&mut self, names: &[&str], call: C)
    where
        C: Fn(&str, &[Value]) -> Result<Value, Error> + Send + Sync + 'static,
    {
        // The closure is shared by every registered name, so one helper set is
        // one allocation rather than one copy of the dispatch per name.
        let call = Arc::new(call);
        for &name in names {
            let call = Arc::clone(&call);
            let helper = name.to_owned();
            self.register(name, move |arguments: &[Value]| call(&helper, arguments));
        }
    }

    /// Registers every name in `names` against one `call` closure, under a group
    /// name the caller chose.
    ///
    /// The group is a label, not a category the engine understands: `"strings"`
    /// and `"mine"` are equally valid, and two groups that share a name simply
    /// accumulate. The names are recorded so [`FuncMap::group`] can report what
    /// was loaded. A second group adds its names; it does not clear the map, and
    /// a name registered twice is replaced.
    ///
    /// # Errors
    ///
    /// Returns [`Error::EmptyGroupName`] for an empty or whitespace-only group
    /// name, and registers nothing in that case. A group with no name could not
    /// be reported, so it is refused rather than silently dropped.
    ///
    /// ```
    /// use neotales_gotmpl::{FuncMap, Template, Value};
    ///
    /// let mut functions = FuncMap::new();
    /// functions.register_group("mine", &["shout", "whisper"], |name, arguments| {
    ///     let text = arguments.first().map(Value::render).unwrap_or_default();
    ///     Ok(Value::String(if name == "shout" {
    ///         text.to_uppercase()
    ///     } else {
    ///         text.to_lowercase()
    ///     }))
    /// })?;
    ///
    /// assert_eq!(functions.group("mine").expect("loaded"), ["shout", "whisper"]);
    ///
    /// let values = [("name".to_owned(), "Acme".to_owned())].into_iter().collect();
    /// let rendered = Template::parse("{{ .name | shout }}")
    ///     .expect("parses")
    ///     .with_funcs(functions)
    ///     .render(&values)
    ///     .expect("renders");
    /// assert_eq!(rendered, "ACME");
    /// # Ok::<(), neotales_gotmpl::Error>(())
    /// ```
    pub fn register_group<C>(&mut self, group: &str, names: &[&str], call: C) -> Result<(), Error>
    where
        C: Fn(&str, &[Value]) -> Result<Value, Error> + Send + Sync + 'static,
    {
        if group.trim().is_empty() {
            return Err(Error::EmptyGroupName);
        }
        self.register_names(names, call);
        let recorded = self.groups.entry(group.to_owned()).or_default();
        for &name in names {
            recorded.push(name.to_owned());
        }
        recorded.sort_unstable();
        recorded.dedup();
        Ok(())
    }

    /// Returns the names recorded under `group`, sorted, or `None` if no group
    /// has that name.
    ///
    /// The report is what was passed to [`FuncMap::register_group`], so a caller
    /// can show the operator which set of functions a template was given.
    #[must_use]
    pub fn group(&self, group: &str) -> Option<&[String]> {
        self.groups.get(group).map(Vec::as_slice)
    }

    /// Returns the recorded group names, in sorted order.
    pub fn groups(&self) -> impl Iterator<Item = &str> {
        self.groups.keys().map(String::as_str)
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
        self.register(name, function);
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
            .field("groups", &self.groups().collect::<Vec<_>>())
            .finish()
    }
}
