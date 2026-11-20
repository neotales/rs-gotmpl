//! Helper groups, matching the sections of Go Sprig's `genericMap`.
//!
//! Go registers its helpers as one flat function map with section comments, and
//! a caller either takes all of it or the hermetic subset. A caller that wants
//! only string helpers has no list to ask for. This module is that list: one
//! group per section of Go's map, containing only the helpers this crate
//! implements, so a caller registers a set of helpers rather than a set of names
//! typed out by hand.
//!
//! The group names are Go's; the membership is filtered to what exists here. A
//! helper Go registers and this crate does not — the crypto, random and network
//! ones — is simply absent, and no group lists a name that [`call`](crate::call)
//! does not know. Two tests enforce both directions.

use crate::helper_names;

/// A set of helpers grouped the way Go Sprig groups them.
///
/// ```
/// use neotales_sprig::{FunctionGroup, group_names};
///
/// let names = group_names(FunctionGroup::Paths);
/// assert!(names.contains(&"base"));
/// assert!(!names.contains(&"upper"));
/// ```
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum FunctionGroup {
    /// `now`, `date`, `toDate`, the duration helpers.
    Dates,
    /// The string manipulation helpers, including quoting and the `sha*sum` digests.
    Strings,
    /// The arithmetic, conversion, `seq`/`until` counters and float helpers.
    ///
    /// Go registers these in the same section as its string helpers. They are
    /// separated here because a caller that wants arithmetic is not asking for
    /// `trim`.
    Numeric,
    /// `default`, `empty`, `ternary`, and the JSON round-trip helpers.
    Defaults,
    /// `typeOf`, `kindOf`, `deepEqual` and the rest of the type inspection set.
    Reflection,
    /// `env` and `expandenv`.
    Os,
    /// `getHostByName`. Empty: this crate makes no network calls, so no helper
    /// here is in it.
    Network,
    /// `base`, `dir`, `clean`, `ext`, `isAbs`.
    Paths,
    /// The `os*` spellings: `osBase`, `osDir`, `osClean`, `osExt`, `osIsAbs`.
    Filepaths,
    /// `b64enc`, `b64dec`, `b32enc`, `b32dec`.
    Encoding,
    /// The list helpers: `list`, `first`, `append`, `join`, `sortAlpha`, `until`.
    Lists,
    /// The dictionary helpers: `dict`, `get`, `set`, `merge`, `keys`.
    Dicts,
    /// Everything in [`Self::Lists`] and [`Self::Dicts`], which Go registers as
    /// one "Data Structures" section.
    ///
    /// This is the combined view, not a separate set: a caller that wants the
    /// whole data-structure section asks for this instead of merging two name
    /// lists.
    DataStructures,
    /// The certificate, password and AES helpers. Empty: they are out of scope
    /// for a crate that promises reproducible output.
    Crypto,
    /// `uuidv4`. Empty: a generated identifier is not reproducible.
    Uuid,
    /// `semver` and `semverCompare`.
    Semver,
    /// `fail`.
    Flow,
    /// The `regex*` and `mustRegex*` helpers.
    Regex,
    /// `urlParse` and `urlJoin`.
    Urls,
}

impl FunctionGroup {
    /// Every group, in the order Go's `genericMap` declares them.
    ///
    /// [`Self::DataStructures`] is not in this list: it is the union of
    /// [`Self::Lists`] and [`Self::Dicts`] rather than a section of its own, and
    /// iterating it as well would register those helpers twice.
    pub const ALL: [Self; 18] = [
        Self::Dates,
        Self::Strings,
        Self::Numeric,
        Self::Defaults,
        Self::Reflection,
        Self::Os,
        Self::Network,
        Self::Paths,
        Self::Filepaths,
        Self::Encoding,
        Self::Lists,
        Self::Dicts,
        Self::Crypto,
        Self::Uuid,
        Self::Semver,
        Self::Flow,
        Self::Regex,
        Self::Urls,
    ];

    /// The group's name, spelled as Go's section comment spells it.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Dates => "dates",
            Self::Strings => "strings",
            Self::Numeric => "numeric",
            Self::Defaults => "defaults",
            Self::Reflection => "reflection",
            Self::Os => "os",
            Self::Network => "network",
            Self::Paths => "paths",
            Self::Filepaths => "filepaths",
            Self::Encoding => "encoding",
            Self::Lists => "lists",
            Self::Dicts => "dicts",
            Self::DataStructures => "data_structures",
            Self::Crypto => "crypto",
            Self::Uuid => "uuid",
            Self::Semver => "semver",
            Self::Flow => "flow",
            Self::Regex => "regex",
            Self::Urls => "urls",
        }
    }
}

impl std::fmt::Display for FunctionGroup {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

const DATES: &[&str] = &[
    "ago",
    "date",
    "dateInZone",
    "dateModify",
    "date_in_zone",
    "date_modify",
    "duration",
    "durationRound",
    "htmlDate",
    "htmlDateInZone",
    "mustDateModify",
    "mustToDate",
    "must_date_modify",
    "now",
    "toDate",
    "unixEpoch",
];

const STRINGS: &[&str] = &[
    "abbrev",
    "abbrevboth",
    "adler32sum",
    "camelcase",
    "cat",
    "contains",
    "hasPrefix",
    "hasSuffix",
    "indent",
    "initials",
    "kebabcase",
    "lower",
    "nindent",
    "nospace",
    "plural",
    "quote",
    "repeat",
    "replace",
    "sha1sum",
    "sha256sum",
    "sha512sum",
    "snakecase",
    "split",
    "splitList",
    "splitn",
    "squote",
    "substr",
    "swapcase",
    "title",
    "toString",
    "toStrings",
    "trim",
    "trimAll",
    "trimPrefix",
    "trimSuffix",
    "trimall",
    "trunc",
    "untitle",
    "upper",
    "wrap",
    "wrapWith",
];

const NUMERIC: &[&str] = &[
    "add",
    "add1",
    "add1f",
    "addf",
    "atoi",
    "biggest",
    "ceil",
    "div",
    "divf",
    "float64",
    "floor",
    "int",
    "int64",
    "max",
    "maxf",
    "min",
    "minf",
    "mod",
    "mul",
    "mulf",
    "round",
    "seq",
    "sub",
    "subf",
    "toDecimal",
    "until",
    "untilStep",
];

const DEFAULTS: &[&str] = &[
    "all",
    "any",
    "coalesce",
    "compact",
    "deepCopy",
    "default",
    "empty",
    "fromJson",
    "mustCompact",
    "mustDeepCopy",
    "mustFromJson",
    "mustToJson",
    "mustToPrettyJson",
    "mustToRawJson",
    "ternary",
    "toJson",
    "toPrettyJson",
    "toRawJson",
];

const REFLECTION: &[&str] = &[
    "deepEqual",
    "kindIs",
    "kindOf",
    "typeIs",
    "typeIsLike",
    "typeOf",
];

const OS: &[&str] = &["env", "expandenv"];

const NETWORK: &[&str] = &[];

const PATHS: &[&str] = &["base", "clean", "dir", "ext", "isAbs"];

const FILEPATHS: &[&str] = &["osBase", "osClean", "osDir", "osExt", "osIsAbs"];

const ENCODING: &[&str] = &["b32dec", "b32enc", "b64dec", "b64enc"];

const LISTS: &[&str] = &[
    "append",
    "chunk",
    "concat",
    "first",
    "has",
    "initial",
    "join",
    "last",
    "list",
    "mustAppend",
    "mustChunk",
    "mustFirst",
    "mustHas",
    "mustInitial",
    "mustLast",
    "mustPrepend",
    "mustPush",
    "mustRest",
    "mustReverse",
    "mustSlice",
    "mustUniq",
    "mustWithout",
    "prepend",
    "push",
    "rest",
    "reverse",
    "seq",
    "slice",
    "sortAlpha",
    "tuple",
    "uniq",
    "until",
    "untilStep",
    "without",
];

const DICTS: &[&str] = &[
    "dict",
    "dig",
    "get",
    "hasKey",
    "keys",
    "merge",
    "mergeOverwrite",
    "mustMerge",
    "mustMergeOverwrite",
    "omit",
    "pick",
    "pluck",
    "set",
    "unset",
    "values",
];

const DATA_STRUCTURES: &[&str] = &[
    "append",
    "chunk",
    "concat",
    "dict",
    "dig",
    "first",
    "get",
    "has",
    "hasKey",
    "initial",
    "join",
    "keys",
    "last",
    "list",
    "merge",
    "mergeOverwrite",
    "mustAppend",
    "mustChunk",
    "mustFirst",
    "mustHas",
    "mustInitial",
    "mustLast",
    "mustMerge",
    "mustMergeOverwrite",
    "mustPrepend",
    "mustPush",
    "mustRest",
    "mustReverse",
    "mustSlice",
    "mustUniq",
    "mustWithout",
    "omit",
    "pick",
    "pluck",
    "prepend",
    "push",
    "rest",
    "reverse",
    "seq",
    "set",
    "slice",
    "sortAlpha",
    "tuple",
    "uniq",
    "unset",
    "until",
    "untilStep",
    "values",
    "without",
];

const CRYPTO: &[&str] = &[];

const UUID: &[&str] = &[];

const SEMVER: &[&str] = &["semver", "semver.Compare", "semverCompare"];

const FLOW: &[&str] = &["fail"];

const REGEX: &[&str] = &[
    "mustRegexFind",
    "mustRegexFindAll",
    "mustRegexMatch",
    "mustRegexReplaceAll",
    "mustRegexReplaceAllLiteral",
    "mustRegexSplit",
    "regexFind",
    "regexFindAll",
    "regexMatch",
    "regexQuoteMeta",
    "regexReplaceAll",
    "regexReplaceAllLiteral",
    "regexSplit",
];

const URLS: &[&str] = &["urlJoin", "urlParse"];

/// Go Sprig's `nonhermeticFunctions`, restricted to the helpers this crate
/// implements, with `ago` added.
///
/// Go's list is the definition of "not guaranteed to give the same result for
/// the same input". It omits `ago`, which reads the clock in Go and here, so
/// keeping `ago` in a set advertised as repeatable would be wrong. The
/// difference from Go's list is exactly that one name and is pinned by a test.
pub const NONHERMETIC: &[&str] = &[
    "ago",
    "date",
    "dateInZone",
    "dateModify",
    "date_in_zone",
    "date_modify",
    "env",
    "expandenv",
    "htmlDate",
    "htmlDateInZone",
    "now",
];

/// The helpers this crate implements in `group`, sorted by name.
///
/// The list is what the crate actually implements, not what Go registers: a name
/// Go has and this crate does not is absent, so registering a whole group can
/// never put a name in a caller's function map that [`call`](crate::call) would
/// then refuse.
///
/// ```
/// use neotales_sprig::{FunctionGroup, group_names};
///
/// assert_eq!(group_names(FunctionGroup::Encoding), ["b32dec", "b32enc", "b64dec", "b64enc"]);
/// assert!(group_names(FunctionGroup::Crypto).is_empty());
/// ```
#[must_use]
pub fn group_names(group: FunctionGroup) -> &'static [&'static str] {
    match group {
        FunctionGroup::Dates => DATES,
        FunctionGroup::Strings => STRINGS,
        FunctionGroup::Numeric => NUMERIC,
        FunctionGroup::Defaults => DEFAULTS,
        FunctionGroup::Reflection => REFLECTION,
        FunctionGroup::Os => OS,
        FunctionGroup::Network => NETWORK,
        FunctionGroup::Paths => PATHS,
        FunctionGroup::Filepaths => FILEPATHS,
        FunctionGroup::Encoding => ENCODING,
        FunctionGroup::Lists => LISTS,
        FunctionGroup::Dicts => DICTS,
        FunctionGroup::DataStructures => DATA_STRUCTURES,
        FunctionGroup::Crypto => CRYPTO,
        FunctionGroup::Uuid => UUID,
        FunctionGroup::Semver => SEMVER,
        FunctionGroup::Flow => FLOW,
        FunctionGroup::Regex => REGEX,
        FunctionGroup::Urls => URLS,
    }
}

/// The implemented helpers that give the same answer for the same input.
///
/// This is [`helper_names`] minus [`NONHERMETIC`]. A caller that renders the same
/// template from the same values twice, in two processes, on two machines, and
/// needs the bytes to match, registers this set rather than the whole one.
///
/// ```
/// use neotales_sprig::hermetic_names;
///
/// let names = hermetic_names();
/// assert!(names.contains(&"upper"));
/// assert!(!names.contains(&"now"));
/// ```
#[must_use]
pub fn hermetic_names() -> Vec<&'static str> {
    helper_names()
        .into_iter()
        .filter(|name| !NONHERMETIC.contains(name))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{FunctionGroup, NONHERMETIC, group_names, hermetic_names};
    use crate::{Error, call, helper_names};

    /// Every group, including the combined data-structure view.
    fn all_groups() -> Vec<FunctionGroup> {
        let mut groups = FunctionGroup::ALL.to_vec();
        groups.push(FunctionGroup::DataStructures);
        groups
    }

    /// A group that names a helper nobody implements would be a promise the
    /// crate breaks at render time, so every listed name must be dispatchable.
    #[test]
    fn every_listed_name_is_a_helper() {
        for group in all_groups() {
            for name in group_names(group) {
                assert!(
                    !matches!(call(name, &[]), Err(Error::UnknownHelper(_))),
                    "{group} lists {name}, which call does not know"
                );
            }
        }
    }

    /// The other direction: a helper that is in no group cannot be registered by
    /// asking for a group, so it would be reachable only by typing its name.
    #[test]
    fn every_helper_is_in_a_group() {
        let mut grouped: Vec<&str> = FunctionGroup::ALL
            .iter()
            .flat_map(|group| group_names(*group))
            .copied()
            .collect();
        grouped.sort_unstable();
        grouped.dedup();
        let implemented = helper_names();
        let missing: Vec<&&str> = implemented
            .iter()
            .filter(|name| !grouped.contains(name))
            .collect();
        assert!(
            missing.is_empty(),
            "{} helpers are in no group: {missing:?}",
            missing.len()
        );
    }

    /// The combined view is exactly the two data-structure groups.
    #[test]
    fn data_structures_is_lists_and_dicts() {
        let mut expected: Vec<&str> = group_names(FunctionGroup::Lists)
            .iter()
            .chain(group_names(FunctionGroup::Dicts))
            .copied()
            .collect();
        expected.sort_unstable();
        expected.dedup();
        assert_eq!(group_names(FunctionGroup::DataStructures), expected);
    }

    /// A name list that is not sorted or repeats a name is a list a reader
    /// cannot check against the reference's registry.
    #[test]
    fn group_names_are_sorted_and_unique() {
        for group in all_groups() {
            let names = group_names(group);
            for pair in names.windows(2) {
                assert!(pair[0] < pair[1], "{group} is not sorted at {pair:?}");
            }
        }
    }

    /// The hermetic set is the full set minus the recorded exceptions, and it
    /// excludes everything that reads the clock or the environment.
    #[test]
    fn hermetic_names_exclude_every_nonhermetic_helper() {
        let hermetic = hermetic_names();
        for name in NONHERMETIC {
            assert!(
                !hermetic.contains(name),
                "{name} is recorded as non-hermetic but is in the hermetic set"
            );
            assert!(
                helper_names().contains(name),
                "{name} is recorded as non-hermetic but is not implemented, so the \
                 record names a helper that does not exist"
            );
        }
        assert_eq!(hermetic.len() + NONHERMETIC.len(), helper_names().len());
    }

    /// The names removed, pinned. Go's own list is this one less `ago`; the
    /// difference is deliberate and documented on [`NONHERMETIC`].
    #[test]
    fn the_nonhermetic_set_is_the_recorded_names() {
        assert_eq!(
            NONHERMETIC,
            [
                "ago",
                "date",
                "dateInZone",
                "dateModify",
                "date_in_zone",
                "date_modify",
                "env",
                "expandenv",
                "htmlDate",
                "htmlDateInZone",
                "now",
            ]
        );
        for name in ["now", "ago", "env"] {
            assert!(!hermetic_names().contains(&name), "{name}");
        }
    }
}
