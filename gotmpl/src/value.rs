//! The value a template expression produces.

use std::collections::BTreeMap;
use std::fmt::Write as _;

/// A value a template expression can produce.
///
/// The distinction that matters is not string versus number but *how a number was
/// written*, because Go's `text/template` tells an integer from a decimal and
/// renders them differently. `{{ 3 }}` is `3`; `{{ 3.0 }}` is `3.0`. That is why
/// this is [`Self::Number`] and [`Self::Float`] rather than one numeric variant.
///
/// This type is the crate's own, so the engine compiles with no helper library
/// and a caller's function map speaks one value type. It is deliberately the
/// same model a Sprig helper set uses, so a caller converting between the two is
/// copying a shape rather than translating a meaning; the conversion lives with
/// the caller, which is what keeps the two libraries independent.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    /// A text value.
    String(String),
    /// A boolean, which is also what a Go template's truthiness rules are
    /// applied to when deciding which branch to take.
    Bool(bool),
    /// A number from a JSON or YAML values file.
    ///
    /// Values files are full of numbers — `"replicas": 3`, `"port": 8080` — and
    /// without this they could not be represented at all, so a template over a
    /// realistic values file would fail to parse. The number is kept as `f64`
    /// because that is what JSON decodes to, and rendered the way Go renders a
    /// `float64`.
    ///
    /// This is the *integer* case. A value that came from a decimal literal or
    /// from a floating-point function is [`Self::Float`] instead, because Go's
    /// `text/template` tells them apart: `{{ 3.0 }}` renders `3.0` where
    /// `{{ 3 }}` renders `3`, and a function that reports a value's kind answers
    /// `int` for one and `float64` for the other. A single `f64` cannot carry
    /// that, and a template choosing a numeric form from a kind would be wrong.
    Number(f64),
    /// A value that came from a decimal literal or a floating-point function.
    ///
    /// The payload is the same `f64`; what differs is the spelling, which a
    /// kind-reporting function can see and which [`Value::render`] must
    /// reproduce.
    Float(f64),
    /// An explicit JSON or YAML null.
    ///
    /// Go renders a nil interface as `<no value>`, and this matches that so a
    /// template over a values file containing a null does not invent a spelling.
    Null,
    /// An ordered sequence.
    List(Vec<Value>),
    /// A keyed collection, rendered in sorted key order so output is stable.
    Map(BTreeMap<String, Value>),
    /// A value that reads as a string but is reached through named members.
    ///
    /// A display string plus members: the display is what separates it from a
    /// plain map, and it is load-bearing rather than cosmetic. A helper that
    /// returns an object whose display is a canonical version can be rendered
    /// and parsed back, where a plain map would render its members instead.
    Object {
        /// How the value reads when it is rendered.
        display: String,
        /// The members reached with a field path.
        members: BTreeMap<String, Value>,
    },
    /// A value that is called rather than read.
    ///
    /// Reading such a field yields something to invoke rather than something to
    /// print: a method names the function it delegates to, and the value it was
    /// reached through is passed as that function's first argument. A helper
    /// that returns an object carrying a method is how a template writes
    /// `{{ (helper "1.2.3").Method }}`.
    Method {
        /// The function the method delegates to, looked up in the caller's
        /// [`FuncMap`](crate::FuncMap) at call time.
        helper: String,
        /// The value the method was reached through, passed as its first
        /// argument.
        receiver: Box<Value>,
    },
}

impl Value {
    /// Reports whether this value is empty for a fallback-style helper.
    ///
    /// A number is empty at zero and null is always empty, matching Go's
    /// emptiness rules for the values a values file can carry. A caller
    /// registering a `default`-shaped function wants exactly this rule.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        match self {
            Self::String(value) => value.is_empty(),
            Self::Bool(value) => !*value,
            Self::Number(value) | Self::Float(value) => *value == 0.0,
            Self::Null => true,
            Self::List(values) => values.is_empty(),
            Self::Map(values) => values.is_empty(),
            // An object is never empty: it is a value with a display string, and
            // an object whose display is empty is still a value.
            Self::Object { display, .. } => display.is_empty(),
            // A method is never empty: it is something to call, and a template
            // that tests it is asking whether it exists.
            Self::Method { .. } => false,
        }
    }

    /// Returns this value as a string slice, or `None` for anything else.
    ///
    /// A function that needs text checks here rather than coercing: a non-string
    /// argument is a caller error, and a silent coercion to `""` turns a typo
    /// into an empty field.
    #[must_use]
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(value) => Some(value),
            _ => None,
        }
    }

    /// Returns the rendered form of this value.
    #[must_use]
    pub fn render(&self) -> String {
        match self {
            Self::String(value) => value.clone(),
            Self::Bool(value) => value.to_string(),
            Self::Number(value) => go_number(*value),
            // A float keeps its decimal spelling even when the value is whole,
            // which is the whole reason this variant exists: `3.0` is what Go
            // writes for a float64 of three, and `3` is what it writes for an
            // int of three.
            Self::Float(value) => go_float(*value),
            Self::Null => "<no value>".to_owned(),
            Self::List(values) => format!(
                "[{}]",
                values
                    .iter()
                    .map(Value::render)
                    .collect::<Vec<_>>()
                    .join(" ")
            ),
            Self::Method { helper, .. } => helper.clone(),
            // The display, not the members: that is the whole reason this is a
            // separate variant from a map.
            Self::Object { display, .. } => display.clone(),
            Self::Map(values) => format!(
                "map[{}]",
                values
                    .iter()
                    .map(|(key, value)| format!("{key}:{}", value.render()))
                    .collect::<Vec<_>>()
                    .join(" ")
            ),
        }
    }

    /// Returns this value as a number, converting the types Go converts.
    ///
    /// Go's `toInt64` accepts a string it can parse and a bool as 1 or 0, and
    /// renders a non-convertible value as zero. A number is returned unchanged.
    #[must_use]
    pub fn as_number(&self) -> f64 {
        match self {
            Self::Number(value) | Self::Float(value) => *value,
            Self::Bool(value) => {
                if *value {
                    1.0
                } else {
                    0.0
                }
            }
            Self::String(value) => value
                .trim()
                .parse::<f64>()
                // A large integer loses its low bits here, which is what Go's
                // `float64` conversion does too: the alternative is to fail on a
                // value a template is entitled to read.
                .or_else(|_| {
                    value.trim().parse::<i64>().map(|number| {
                        #[allow(clippy::cast_precision_loss)]
                        let number = number as f64;
                        number
                    })
                })
                .unwrap_or(0.0),
            _ => 0.0,
        }
    }

    /// Returns this value as a boolean, following Go's truth rules.
    #[must_use]
    pub fn as_bool(&self) -> bool {
        match self {
            Self::Bool(value) => *value,
            Self::String(value) => value == "true",
            Self::Number(value) | Self::Float(value) => *value != 0.0,
            Self::Null => false,
            Self::List(values) => !values.is_empty(),
            Self::Map(values) => !values.is_empty(),
            // An object and a method are both truthy: each is something to reach
            // into or call, so a template testing whether one is present must not
            // be told the value is empty.
            Self::Object { .. } | Self::Method { .. } => true,
        }
    }

    /// Returns the elements of a list, or a single-element list for a scalar.
    ///
    /// A list helper that accepts a scalar as a one-element list is what makes
    /// `append "a" "b"` and `append (list "a") "b"` both spell a two element
    /// list, so this is the shape such a function starts from.
    #[must_use]
    pub fn as_list(&self) -> Vec<Value> {
        match self {
            Self::List(values) => values.clone(),
            // A null and a method are both empty: a null has no elements, and a
            // method is not a one-element list, because a function that walks a
            // list must not find a callable in it.
            Self::Null | Self::Method { .. } => Vec::new(),
            other => vec![other.clone()],
        }
    }
}

fn go_float(value: f64) -> String {
    let rendered = go_number(value);
    // Go's `%v` for a `float64` keeps a fractional part, so a whole float still
    // reads as a float. The scientific form already carries a point, and a
    // negative zero keeps its sign, so neither needs a point added.
    if rendered.contains('.') || rendered.contains('e') || rendered.contains("Inf") {
        return rendered;
    }
    format!("{rendered}.0")
}

/// Renders a number the way Go's `text/template` renders a `float64`.
///
/// Go prints the shortest representation that round-trips and switches to
/// scientific notation when the decimal exponent is below `-4` or at least `6`.
/// Rust's default formatting never switches, so `1000000` would render as
/// `1000000` where Go writes `1e+06`, and `1e21` as twenty-one digits where Go
/// writes `1e+21`. A template over a values file should not depend on which
/// formatting the implementation happened to choose, so Go's rule is reproduced
/// exactly.
///
/// The shortest round-tripping digits and the exponent are read back out of
/// Rust's own `{:e}` formatting, which is the same shortest representation, and
/// then laid out by Go's rule.
fn go_number(value: f64) -> String {
    if value.is_nan() {
        return "NaN".to_owned();
    }
    if value.is_infinite() {
        return if value.is_sign_positive() {
            "+Inf".to_owned()
        } else {
            "-Inf".to_owned()
        };
    }

    // `{:e}` yields the shortest round-tripping mantissa and a decimal exponent,
    // e.g. `1.234567e6`. Zero is the one value it renders as `0e0`.
    let scientific = format!("{value:e}");
    let (mantissa, exponent) = scientific
        .split_once('e')
        .expect("Rust's exponential formatting always has an exponent");
    let exponent: i32 = exponent
        .parse()
        .expect("Rust's exponential formatting writes a decimal exponent");
    let negative = mantissa.starts_with('-');
    let digits: String = mantissa.chars().filter(char::is_ascii_digit).collect();
    let digits = digits.trim_end_matches('0');
    let digits = if digits.is_empty() { "0" } else { digits };

    if value == 0.0 {
        return if negative {
            "-0".to_owned()
        } else {
            "0".to_owned()
        };
    }

    // Go's rule: scientific when the exponent is below -4 or at least 6.
    if !(-4..6).contains(&exponent) {
        let sign = if negative { "-" } else { "" };
        let mut out = String::from(sign);
        out.push_str(&digits[..1]);
        if digits.len() > 1 {
            out.push('.');
            out.push_str(&digits[1..]);
        }
        // Go's exponent always carries a sign and at least two digits.
        out.push('e');
        out.push(if exponent < 0 { '-' } else { '+' });
        // Go writes at least two exponent digits, so `1e+06` is not `1e+6`.
        let _ = write!(out, "{:02}", exponent.unsigned_abs());
        return out;
    }

    // Decimal layout. `exponent` is the power of ten of the leading digit, so a
    // negative exponent puts the point before the digits and needs that many
    let sign = if negative { "-" } else { "" };
    if exponent < 0 {
        // Both conversions are exact: the branch is reached only for a negative
        // exponent, and the point index is only read for a non-negative one.
        #[allow(clippy::cast_sign_loss)]
        let zeros = exponent.unsigned_abs() as usize;
        return format!("{sign}0.{}{}", "0".repeat(zeros - 1), digits);
    }
    #[allow(clippy::cast_sign_loss)]
    let point = exponent as usize + 1;
    if point >= digits.len() {
        return format!("{sign}{digits}{}", "0".repeat(point - digits.len()));
    }
    format!("{sign}{}.{}", &digits[..point], &digits[point..])
}
