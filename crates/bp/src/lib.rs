//! Parser for the data subset of the Lua dialect that blueprint and map
//! files use.
//!
//! The dialect is Lua 5.0 with the original developer's changes. The files
//! this crate reads use only data: a constructor call wrapping a table of
//! literals, or a sequence of top-level `name = value` assignments, with
//! constructor calls (`VECTOR3( 1, 2, 3 )`, `Sound { Bank = 'x' }`) as
//! values. Nothing is evaluated: a constructor call is kept as
//! [`Value::Call`], and a file that needs a function, an expression, or
//! control flow is an [`Error::Unsupported`], never a feature request.
//!
//! This is the skeleton: every parse function returns
//! [`Error::Unimplemented`].

use std::fmt;

/// A parsed value.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    /// `nil`.
    Nil,
    /// `true` or `false`.
    Bool(bool),
    /// A numeral; an integer literal and its decimal form compare equal.
    Number(f64),
    /// A quoted string, escapes resolved.
    Str(String),
    /// A localised string, `<LOC id>text`.
    Loc {
        /// The key between the angle brackets.
        id: String,
        /// The text after the closing bracket.
        text: String,
    },
    /// A table constructor; positional entries are keyed [`Key::Index`] from
    /// 1 in order.
    Table(Vec<(Key, Value)>),
    /// A constructor call such as `VECTOR3( 1, 2, 3 )` or `Sound { .. }`,
    /// kept as written; a brace-form call has the table as its one argument.
    Call {
        /// The called identifier.
        name: String,
        /// The arguments in order, never evaluated.
        args: Vec<Value>,
    },
}

/// A table key.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Key {
    /// A positional entry (numbered from 1) or a bracketed number.
    Index(usize),
    /// A bare name or a bracketed string.
    Name(String),
}

/// A blueprint file: its constructor and the table it wraps.
#[derive(Clone, Debug, PartialEq)]
pub struct Blueprint {
    /// The constructor identifier, such as `UnitBlueprint`.
    pub constructor: String,
    /// The wrapped value, a [`Value::Table`] in every installed file.
    pub body: Value,
}

/// A named point from a scenario save file.
#[derive(Clone, Debug, PartialEq)]
pub struct Marker {
    /// The entry's key in the Markers table.
    pub name: String,
    /// The string argument of its `type` call.
    pub kind: String,
    /// The three numbers of its `position` call.
    pub position: [f32; 3],
}

/// Why a file did not parse.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Error {
    /// The text is not in the grammar.
    Syntax {
        /// One-based line of the offence.
        line: u32,
        /// One-based column of the offence.
        column: u32,
        /// What was wrong.
        reason: String,
    },
    /// The text is Lua but not the data subset.
    Unsupported {
        /// One-based line of the construct.
        line: u32,
        /// The construct, such as `function`.
        construct: String,
    },
    /// The bytes could not be decoded.
    Encoding {
        /// What was wrong.
        reason: String,
    },
    /// Reserved for the skeleton: the operation is not built yet.
    Unimplemented,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Syntax {
                line,
                column,
                reason,
            } => write!(f, "syntax error at {line}:{column}: {reason}"),
            Self::Unsupported { line, construct } => {
                write!(f, "unsupported construct at line {line}: {construct}")
            }
            Self::Encoding { reason } => write!(f, "encoding error: {reason}"),
            Self::Unimplemented => f.write_str("not implemented"),
        }
    }
}

impl std::error::Error for Error {}

impl Value {
    /// The value of the entry named `name` when this is a table.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&Value> {
        let _ = name;
        None
    }

    /// The number when this is a [`Value::Number`].
    #[must_use]
    pub fn as_f64(&self) -> Option<f64> {
        None
    }

    /// The text when this is a [`Value::Str`].
    #[must_use]
    pub fn as_str(&self) -> Option<&str> {
        None
    }

    /// The entries when this is a table, else an empty slice.
    #[must_use]
    pub fn entries(&self) -> &[(Key, Value)] {
        &[]
    }
}

/// Parses a blueprint file: a constructor call wrapping a table.
///
/// # Errors
///
/// [`Error::Syntax`] if the text is not in the grammar;
/// [`Error::Unsupported`] if it needs more than the data subset.
pub fn parse(text: &str) -> Result<Blueprint, Error> {
    let _ = text;
    Err(Error::Unimplemented)
}

/// Parses a blueprint file from bytes, decoding ISO-8859-1.
///
/// # Errors
///
/// As [`parse`], and [`Error::Encoding`] if the bytes cannot be decoded.
pub fn parse_bytes(bytes: &[u8]) -> Result<Blueprint, Error> {
    let _ = bytes;
    Err(Error::Unimplemented)
}

/// Parses a script: a sequence of top-level `name = value` assignments, as
/// the map Lua files are.
///
/// # Errors
///
/// As [`parse`].
pub fn parse_script(text: &str) -> Result<Vec<(String, Value)>, Error> {
    let _ = text;
    Err(Error::Unimplemented)
}

/// Every entry of `Scenario.MasterChain['_MASTERCHAIN_'].Markers` that is a
/// table with a `type` call `STRING( kind )` and a `position` call
/// `VECTOR3( x, y, z )`; an entry lacking either is skipped.
///
/// # Errors
///
/// [`Error::Unimplemented`] in the skeleton.
pub fn markers(scenario_save: &[(String, Value)]) -> Result<Vec<Marker>, Error> {
    let _ = scenario_save;
    Err(Error::Unimplemented)
}
