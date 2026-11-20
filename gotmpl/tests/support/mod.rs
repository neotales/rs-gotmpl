//! Loading the Sprig helper set into `neotales-gotmpl`.
//!
//! The two libraries do not depend on each other by default: `neotales-sprig` is
//! a function set that names no engine, and `neotales-gotmpl` is an engine that
//! names no helper set. The glue between them is `neotales-sprig`'s `gotmpl`
//! feature, which this crate turns on for its tests, so there is exactly one copy
//! of the value conversion and it lives in the crate that owns one of the two
//! types.
#![allow(dead_code)]

use neotales_gotmpl::FuncMap;
use neotales_sprig::FunctionGroup;

/// Every helper this crate implements.
pub(crate) fn sprig_functions() -> FuncMap {
    let mut functions = FuncMap::new();
    neotales_sprig::register(&mut functions, &neotales_sprig::helper_names());
    functions
}

/// The helpers in one Go Sprig group.
pub(crate) fn group_functions(group: FunctionGroup) -> FuncMap {
    let mut functions = FuncMap::new();
    neotales_sprig::register_group(&mut functions, group);
    functions
}

/// The helpers whose answer depends on nothing but their arguments.
pub(crate) fn hermetic_functions() -> FuncMap {
    let mut functions = FuncMap::new();
    neotales_sprig::register_hermetic(&mut functions);
    functions
}
