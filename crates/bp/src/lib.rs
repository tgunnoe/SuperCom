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
        }
    }
}

impl std::error::Error for Error {}

impl Value {
    /// The value of the entry named `name` when this is a table.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&Value> {
        self.entries().iter().find_map(|(key, value)| match key {
            Key::Name(n) if n == name => Some(value),
            _ => None,
        })
    }

    /// The number when this is a [`Value::Number`].
    #[must_use]
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Self::Number(n) => Some(*n),
            _ => None,
        }
    }

    /// The text when this is a [`Value::Str`].
    #[must_use]
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::Str(s) => Some(s),
            _ => None,
        }
    }

    /// The entries when this is a table, else an empty slice.
    #[must_use]
    pub fn entries(&self) -> &[(Key, Value)] {
        match self {
            Self::Table(entries) => entries,
            _ => &[],
        }
    }
}

/// Deepest table or call nesting the parser follows before it reports a
/// syntax error; the installed files nest a handful of levels.
const MAX_DEPTH: usize = 256;

/// Words of Lua that are not part of the data subset.
const KEYWORDS: &[&str] = &[
    "and", "break", "continue", "do", "else", "elseif", "end", "for", "function", "if", "in",
    "local", "not", "or", "repeat", "return", "then", "until", "while",
];

/// A recursive-descent reader over the decoded characters of one file.
///
/// The grammar is that of the data subset: values are literals, tables, and
/// constructor calls; a table holds `name = value`, `[key] = value`, and
/// positional entries separated by `,` or `;`. Line and column are derived
/// from the character offset only when an error is built.
struct Reader {
    chars: Vec<char>,
    at: usize,
}

impl Reader {
    fn new(text: &str) -> Self {
        Self {
            chars: text.chars().collect(),
            at: 0,
        }
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.at).copied()
    }

    fn peek_at(&self, ahead: usize) -> Option<char> {
        self.chars.get(self.at + ahead).copied()
    }

    /// One-based line and column of the character offset `at`.
    fn position(&self, at: usize) -> (u32, u32) {
        let mut line = 1usize;
        let mut column = 1usize;
        for &c in self.chars.iter().take(at) {
            if c == '\n' {
                line += 1;
                column = 1;
            } else {
                column += 1;
            }
        }
        (
            u32::try_from(line).unwrap_or(u32::MAX),
            u32::try_from(column).unwrap_or(u32::MAX),
        )
    }

    fn syntax(&self, at: usize, reason: impl Into<String>) -> Error {
        let (line, column) = self.position(at);
        Error::Syntax {
            line,
            column,
            reason: reason.into(),
        }
    }

    fn unsupported(&self, at: usize, construct: impl Into<String>) -> Error {
        let (line, _) = self.position(at);
        Error::Unsupported {
            line,
            construct: construct.into(),
        }
    }

    /// Skips whitespace and the comment forms `#`, `--`, `//` (to the end
    /// of the line) and `--[[ .. ]]`, `/* .. */` (to the closer).
    fn skip(&mut self) -> Result<(), Error> {
        loop {
            match (self.peek(), self.peek_at(1)) {
                (Some(c), _) if c.is_ascii_whitespace() => self.at += 1,
                (Some('#'), _) | (Some('-'), Some('-')) | (Some('/'), Some('/')) => {
                    let block = self.peek() == Some('-')
                        && self.peek_at(2) == Some('[')
                        && self.peek_at(3) == Some('[');
                    if block {
                        self.block_comment(4, "]]")?;
                    } else {
                        while self.peek().is_some_and(|c| c != '\n') {
                            self.at += 1;
                        }
                    }
                }
                (Some('/'), Some('*')) => self.block_comment(2, "*/")?,
                _ => return Ok(()),
            }
        }
    }

    fn block_comment(&mut self, open_len: usize, closer: &str) -> Result<(), Error> {
        let start = self.at;
        self.at += open_len;
        let closer: Vec<char> = closer.chars().collect();
        while self.at < self.chars.len() {
            if self.chars.get(self.at..self.at + closer.len()) == Some(closer.as_slice()) {
                self.at += closer.len();
                return Ok(());
            }
            self.at += 1;
        }
        Err(self.syntax(start, "unterminated block comment"))
    }

    fn expect(&mut self, want: char) -> Result<(), Error> {
        if self.peek() == Some(want) {
            self.at += 1;
            Ok(())
        } else {
            Err(self.unexpected(&format!("expected `{want}`")))
        }
    }

    fn unexpected(&self, expected: &str) -> Error {
        match self.peek() {
            None => self.syntax(self.at, format!("{expected}, found end of input")),
            Some('\0') => self.syntax(self.at, "NUL byte in the text"),
            Some(c) if is_operator(c) => {
                self.unsupported(self.at, format!("operator `{c}` ({expected})"))
            }
            Some(c) => self.syntax(self.at, format!("{expected}, found `{c}`")),
        }
    }

    fn ident(&mut self) -> String {
        let start = self.at;
        while self.peek().is_some_and(is_ident_char) {
            self.at += 1;
        }
        self.chars[start..self.at].iter().collect()
    }

    /// Whether the next token, after skipping blanks, is a single `=`.
    fn assignment_follows(&mut self) -> Result<bool, Error> {
        let saved = self.at;
        self.skip()?;
        let yes = self.peek() == Some('=') && self.peek_at(1) != Some('=');
        self.at = saved;
        Ok(yes)
    }

    fn value(&mut self, depth: usize) -> Result<Value, Error> {
        if depth > MAX_DEPTH {
            return Err(self.syntax(self.at, "nesting is too deep"));
        }
        self.skip()?;
        let start = self.at;
        match self.peek() {
            Some('{') => self.table(depth),
            Some(q @ ('\'' | '"')) => self.string(q),
            Some(c) if c.is_ascii_digit() || c == '-' || c == '.' => self.number(),
            Some(c) if is_ident_start(c) => {
                let name = self.ident();
                match name.as_str() {
                    "nil" => Ok(Value::Nil),
                    "true" => Ok(Value::Bool(true)),
                    "false" => Ok(Value::Bool(false)),
                    word if KEYWORDS.contains(&word) => Err(self.unsupported(start, word)),
                    _ => self.call(name, start, depth),
                }
            }
            _ => Err(self.unexpected("expected a value")),
        }
    }

    /// An identifier in value position must be a constructor call, either
    /// `Name( args )` or `Name { table }`.
    fn call(&mut self, name: String, start: usize, depth: usize) -> Result<Value, Error> {
        self.skip()?;
        match self.peek() {
            Some('(') => {
                self.at += 1;
                let mut args = Vec::new();
                loop {
                    self.skip()?;
                    if self.peek() == Some(')') {
                        self.at += 1;
                        break;
                    }
                    args.push(self.value(depth + 1)?);
                    self.skip()?;
                    match self.peek() {
                        Some(',') => self.at += 1,
                        Some(')') => {}
                        _ => return Err(self.unexpected("expected `,` or `)`")),
                    }
                }
                Ok(Value::Call { name, args })
            }
            Some('{') => {
                let table = self.table(depth)?;
                Ok(Value::Call {
                    name,
                    args: vec![table],
                })
            }
            _ => Err(self.unsupported(start, format!("bare identifier `{name}`"))),
        }
    }

    fn table(&mut self, depth: usize) -> Result<Value, Error> {
        self.expect('{')?;
        let mut entries = Vec::new();
        let mut next_index = 1usize;
        loop {
            self.skip()?;
            if self.peek() == Some('}') {
                self.at += 1;
                return Ok(Value::Table(entries));
            }
            let key_start = self.at;
            let entry = match self.peek() {
                Some('[') => {
                    self.at += 1;
                    let key = self.bracket_key(key_start)?;
                    self.skip()?;
                    self.expect(']')?;
                    self.skip()?;
                    self.expect('=')?;
                    (key, self.value(depth + 1)?)
                }
                Some(c) if is_ident_start(c) && self.named_entry_follows()? => {
                    let name = self.ident();
                    if KEYWORDS.contains(&name.as_str()) {
                        return Err(self.unsupported(key_start, name));
                    }
                    self.skip()?;
                    self.expect('=')?;
                    (Key::Name(name), self.value(depth + 1)?)
                }
                _ => {
                    let value = self.value(depth + 1)?;
                    let key = Key::Index(next_index);
                    next_index += 1;
                    (key, value)
                }
            };
            entries.push(entry);
            self.skip()?;
            match self.peek() {
                Some(',' | ';') => self.at += 1,
                Some('}') => {}
                _ => return Err(self.unexpected("expected `,` or `}`")),
            }
        }
    }

    /// Whether the identifier at the cursor is a `name =` entry key.
    fn named_entry_follows(&mut self) -> Result<bool, Error> {
        let saved = self.at;
        let _ = self.ident();
        let yes = self.assignment_follows()?;
        self.at = saved;
        Ok(yes)
    }

    fn bracket_key(&mut self, start: usize) -> Result<Key, Error> {
        self.skip()?;
        match self.peek() {
            Some(q @ ('\'' | '"')) => match self.string(q)? {
                Value::Str(name) => Ok(Key::Name(name)),
                Value::Loc { id, text } => Ok(Key::Name(format!("<LOC {id}>{text}"))),
                _ => Err(self.syntax(start, "bad bracketed key")),
            },
            Some(c) if c.is_ascii_digit() => {
                let at = self.at;
                let number = self.number()?;
                match number {
                    Value::Number(n) if n >= 0.0 && n.fract() == 0.0 && n < 1e15 => {
                        #[expect(
                            clippy::cast_possible_truncation,
                            clippy::cast_sign_loss,
                            reason = "range-checked above: a non-negative integer below 1e15"
                        )]
                        let index = n as usize;
                        Ok(Key::Index(index))
                    }
                    _ => Err(self.syntax(at, "a numeric key must be a non-negative integer")),
                }
            }
            _ => Err(self.unexpected("expected a string or number key")),
        }
    }

    fn number(&mut self) -> Result<Value, Error> {
        let start = self.at;
        let mut text = String::new();
        if self.peek() == Some('-') {
            text.push('-');
            self.at += 1;
        }
        let digits_from = self.at;
        while let Some(c) = self.peek().filter(char::is_ascii_digit) {
            text.push(c);
            self.at += 1;
        }
        if self.peek() == Some('.') {
            text.push('.');
            self.at += 1;
            while let Some(c) = self.peek().filter(char::is_ascii_digit) {
                text.push(c);
                self.at += 1;
            }
        }
        if self.at == digits_from || text.ends_with('-') || text == "-." || text == "." {
            return Err(match self.peek() {
                Some(c) if is_ident_start(c) => {
                    self.unsupported(start, "unary minus on a non-literal")
                }
                _ => self.syntax(start, "malformed number"),
            });
        }
        if matches!(self.peek(), Some('e' | 'E')) {
            let marker = self.at;
            text.push('e');
            self.at += 1;
            if let Some(sign @ ('+' | '-')) = self.peek() {
                text.push(sign);
                self.at += 1;
            }
            let exponent_from = self.at;
            while let Some(c) = self.peek().filter(char::is_ascii_digit) {
                text.push(c);
                self.at += 1;
            }
            if self.at == exponent_from {
                return Err(self.syntax(marker, "malformed exponent"));
            }
        }
        if self.peek().is_some_and(is_ident_char) {
            return Err(self.syntax(start, "malformed number (hexadecimal is not supported)"));
        }
        text.parse::<f64>()
            .map(Value::Number)
            .map_err(|_| self.syntax(start, "malformed number"))
    }

    fn string(&mut self, quote: char) -> Result<Value, Error> {
        let start = self.at;
        self.at += 1;
        let mut out = String::new();
        loop {
            let Some(c) = self.peek() else {
                return Err(self.syntax(start, "unterminated string"));
            };
            let here = self.at;
            self.at += 1;
            match c {
                c if c == quote => break,
                '\n' | '\r' => return Err(self.syntax(here, "unterminated string")),
                '\0' => return Err(self.syntax(here, "NUL byte in the text")),
                '\\' => out.push(self.escape(here)?),
                c => out.push(c),
            }
        }
        Ok(loc_or_str(out))
    }

    /// Resolves the escape after a backslash at `backslash`: the Lua 5.0 set
    /// of single-character escapes and `\ddd` decimal bytes.
    fn escape(&mut self, backslash: usize) -> Result<char, Error> {
        let Some(c) = self.peek() else {
            return Err(self.syntax(backslash, "unterminated escape"));
        };
        self.at += 1;
        let simple = match c {
            'a' => Some('\u{7}'),
            'b' => Some('\u{8}'),
            'f' => Some('\u{c}'),
            'n' | '\n' => Some('\n'),
            'r' => Some('\r'),
            't' => Some('\t'),
            'v' => Some('\u{b}'),
            '\\' | '"' | '\'' => Some(c),
            _ => None,
        };
        if let Some(simple) = simple {
            return Ok(simple);
        }
        if c.is_ascii_digit() {
            let mut value = c.to_digit(10).unwrap_or(0);
            for _ in 0..2 {
                match self.peek().and_then(|d| d.to_digit(10)) {
                    Some(d) => {
                        value = value * 10 + d;
                        self.at += 1;
                    }
                    None => break,
                }
            }
            return u8::try_from(value)
                .map(char::from)
                .map_err(|_| self.syntax(backslash, "decimal escape above 255"));
        }
        Err(self.syntax(backslash, format!("unknown escape `\\{c}`")))
    }

    /// Reads `name = value` assignments to the end of the text.
    fn assignments(&mut self) -> Result<Vec<(String, Value)>, Error> {
        let mut out = Vec::new();
        loop {
            self.skip()?;
            match self.peek() {
                None => return Ok(out),
                Some(';') => self.at += 1,
                Some(c) if is_ident_start(c) => {
                    let start = self.at;
                    let name = self.ident();
                    if KEYWORDS.contains(&name.as_str()) {
                        return Err(self.unsupported(start, name));
                    }
                    self.skip()?;
                    self.expect('=')?;
                    out.push((name, self.value(0)?));
                }
                Some(_) => return Err(self.unexpected("expected an assignment")),
            }
        }
    }
}

fn is_ident_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_'
}

fn is_ident_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

fn is_operator(c: char) -> bool {
    matches!(c, '+' | '*' | '/' | '^' | '%' | '<' | '>' | '~' | '!')
}

/// Splits `<LOC id>text` into a [`Value::Loc`]; any other text is a plain
/// string.
fn loc_or_str(text: String) -> Value {
    if let Some(rest) = text.strip_prefix("<LOC ") {
        if let Some((id, body)) = rest.split_once('>') {
            return Value::Loc {
                id: id.trim().to_owned(),
                text: body.to_owned(),
            };
        }
    }
    Value::Str(text)
}

/// Parses a blueprint file: a constructor call wrapping a table.
///
/// # Errors
///
/// [`Error::Syntax`] if the text is not in the grammar;
/// [`Error::Unsupported`] if it needs more than the data subset.
pub fn parse(text: &str) -> Result<Blueprint, Error> {
    let mut reader = Reader::new(text);
    reader.skip()?;
    let start = reader.at;
    if !reader.peek().is_some_and(is_ident_start) {
        return Err(reader.unexpected("expected a constructor name"));
    }
    let constructor = reader.ident();
    if KEYWORDS.contains(&constructor.as_str()) {
        return Err(reader.unsupported(start, constructor));
    }
    reader.skip()?;
    let body = reader.table(0)?;
    reader.skip()?;
    if reader.peek() == Some(';') {
        reader.at += 1;
        reader.skip()?;
    }
    if reader.peek().is_some() {
        return Err(reader.unexpected("expected the end of the file"));
    }
    Ok(Blueprint { constructor, body })
}

/// Parses a blueprint file from bytes, decoding ISO-8859-1.
///
/// # Errors
///
/// As [`parse`], and [`Error::Encoding`] if the bytes cannot be decoded.
pub fn parse_bytes(bytes: &[u8]) -> Result<Blueprint, Error> {
    let text: String = bytes.iter().map(|&b| char::from(b)).collect();
    parse(&text)
}

/// Parses a script: a sequence of top-level `name = value` assignments, as
/// the map Lua files are.
///
/// # Errors
///
/// As [`parse`].
pub fn parse_script(text: &str) -> Result<Vec<(String, Value)>, Error> {
    Reader::new(text).assignments()
}

/// Every entry of `Scenario.MasterChain['_MASTERCHAIN_'].Markers` that is a
/// table with a `type` call `STRING( kind )` and a `position` call
/// `VECTOR3( x, y, z )`; an entry lacking either is skipped.
///
/// # Errors
///
/// None today: a save without the markers table yields no markers. The
/// `Result` keeps room for a stricter reading.
pub fn markers(scenario_save: &[(String, Value)]) -> Result<Vec<Marker>, Error> {
    let table = scenario_save
        .iter()
        .find(|(name, _)| name == "Scenario")
        .and_then(|(_, scenario)| scenario.get("MasterChain"))
        .and_then(|chain| chain.get("_MASTERCHAIN_"))
        .and_then(|master| master.get("Markers"));
    let Some(table) = table else {
        return Ok(Vec::new());
    };
    Ok(table
        .entries()
        .iter()
        .filter_map(|(key, entry)| match key {
            Key::Name(name) => marker(name, entry),
            Key::Index(_) => None,
        })
        .collect())
}

fn marker(name: &str, entry: &Value) -> Option<Marker> {
    let kind = match entry.get("type")? {
        Value::Call { name, args } if name == "STRING" => args.first()?.as_str()?.to_owned(),
        _ => return None,
    };
    let position = match entry.get("position")? {
        Value::Call { name, args } if name == "VECTOR3" => args,
        _ => return None,
    };
    let [x, y, z] = position.as_slice() else {
        return None;
    };
    #[expect(
        clippy::cast_possible_truncation,
        reason = "marker positions are stored as f32 by design"
    )]
    let narrow = |v: &Value| v.as_f64().map(|n| n as f32);
    Some(Marker {
        name: name.to_owned(),
        kind,
        position: [narrow(x)?, narrow(y)?, narrow(z)?],
    })
}
