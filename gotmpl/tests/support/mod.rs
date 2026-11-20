//! Loading the Sprig helper set into `neotales-gotmpl`.
//!
//! The two libraries do not depend on each other. `neotales-sprig` is a function
//! set that names no engine, and `neotales-gotmpl` is an engine that names no
//! helper set. The value conversion between them is `adapter`, a workspace crate
//! that is not published, so this module and the rest of the tests share one
//! copy of it instead of each writing their own.
#![allow(dead_code)]

use neotales_gotmpl::FuncMap;
use neotales_sprig::FunctionGroup;

/// Every helper the crate implements, loaded as one group.
pub(crate) fn sprig_functions() -> FuncMap {
    let mut functions = FuncMap::new();
    functions
        .register_group("sprig", &neotales_sprig::helper_names(), adapter::call)
        .expect("the group name is not empty");
    functions
}

/// The helpers in one Go Sprig group.
pub(crate) fn group_functions(group: FunctionGroup) -> FuncMap {
    let mut functions = FuncMap::new();
    functions
        .register_group(
            group.as_str(),
            neotales_sprig::group_names(group),
            adapter::call,
        )
        .expect("a group name from FunctionGroup::as_str is not empty");
    functions
}

/// The helpers whose answer depends on nothing but their arguments.
pub(crate) fn hermetic_functions() -> FuncMap {
    let mut functions = FuncMap::new();
    functions
        .register_group("hermetic", &neotales_sprig::hermetic_names(), adapter::call)
        .expect("the group name is not empty");
    functions
}
