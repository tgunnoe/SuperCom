//! Synthetic suite of `supercom-bp`: every input is text built here from the
//! dialect's grammar with invented identifiers and numbers. No archive and no
//! map is opened. Against the skeleton every test fails on its parse
//! returning `Err`.

#![allow(
    clippy::expect_used,
    reason = "this file is entirely test code; clippy.toml exempts only #[test] functions, not their helpers"
)]

use supercom_bp::{Error, Key, Marker, Value, markers, parse, parse_bytes, parse_script};

/// Parses a blueprint text, failing the calling test with the error.
fn parsed(text: &str) -> supercom_bp::Blueprint {
    let result = parse(text);
    assert!(
        result.is_ok(),
        "parse must accept the text, got {:?}",
        result.as_ref().err()
    );
    result.expect("parsed: asserted above")
}

/// Parses a script text, failing the calling test with the error.
fn scripted(text: &str) -> Vec<(String, Value)> {
    let result = parse_script(text);
    assert!(
        result.is_ok(),
        "parse_script must accept the text, got {:?}",
        result.as_ref().err()
    );
    result.expect("scripted: asserted above")
}

fn num(n: f64) -> Value {
    Value::Number(n)
}

fn text(s: &str) -> Value {
    Value::Str(s.to_owned())
}

/// Pins the shape: the constructor name is kept and the body is the table,
/// with nested tables and named entries in source order.
#[test]
fn constructor_wraps_a_nested_table() {
    let bp =
        parsed("FooBlueprint {\n  Alpha = 1,\n  Beta = { Gamma = 'x', Delta = { Eps = 2 } },\n}");
    assert_eq!(bp.constructor, "FooBlueprint");
    let beta = bp.body.get("Beta").expect("Beta present");
    assert_eq!(beta.get("Gamma").and_then(Value::as_str), Some("x"));
    assert_eq!(
        beta.get("Delta")
            .and_then(|d| d.get("Eps"))
            .and_then(Value::as_f64),
        Some(2.0)
    );
    assert_eq!(bp.body.entries().len(), 2);
}

/// Pins the positional rule: unnamed entries are keyed `Index` from 1 in
/// order, and a named entry between them does not consume an index.
#[test]
fn positional_entries_are_numbered_from_one() {
    let bp = parsed("FooBlueprint { Items = { 'a', 'b', Named = 9, 'c' } }");
    let items = bp.body.get("Items").expect("Items present");
    assert_eq!(
        items.entries(),
        &[
            (Key::Index(1), text("a")),
            (Key::Index(2), text("b")),
            (Key::Name("Named".into()), num(9.0)),
            (Key::Index(3), text("c")),
        ]
    );
}

/// Pins the grammar: a trailing comma is accepted in a table, nested or not,
/// and adds no entry.
#[test]
fn trailing_commas_are_accepted() {
    let bp = parsed("FooBlueprint { A = { 1, 2, }, B = 3, }");
    assert_eq!(bp.body.entries().len(), 2);
    assert_eq!(bp.body.get("A").map(|a| a.entries().len()), Some(2));
}

/// Pins the comments: `#` and `--` line comments are whitespace, wherever a
/// line may end, and a comment marker inside a string is not a comment.
#[test]
fn hash_and_dash_comments_are_skipped() {
    let bp = parsed(
        "# leading\nFooBlueprint { -- after the brace\n  A = 1, # after a value\n  B = 'not # a -- comment',\n}\n-- trailing",
    );
    assert_eq!(bp.body.get("A").and_then(Value::as_f64), Some(1.0));
    assert_eq!(
        bp.body.get("B").and_then(Value::as_str),
        Some("not # a -- comment")
    );
}

/// Pins the escapes: `\n`, `\\`, `\"`, and `\'` resolve in both quote
/// styles, and a double-quoted string may hold a bare single quote.
#[test]
fn string_escapes_resolve_in_both_quote_styles() {
    let bp =
        parsed(r#"FooBlueprint { A = 'a\nb', B = 'c\\d', C = "e\"f", D = 'g\'h', E = "it's" }"#);
    let get = |k: &str| bp.body.get(k).and_then(Value::as_str).map(str::to_owned);
    assert_eq!(get("A").as_deref(), Some("a\nb"));
    assert_eq!(get("B").as_deref(), Some("c\\d"));
    assert_eq!(get("C").as_deref(), Some("e\"f"));
    assert_eq!(get("D").as_deref(), Some("g'h"));
    assert_eq!(get("E").as_deref(), Some("it's"));
}

/// Pins the convention that an escape the parser does not know is a syntax
/// error, not a silent pass-through.
#[test]
fn unknown_escape_is_a_syntax_error() {
    let result = parse(r"FooBlueprint { A = 'bad\qescape' }");
    assert!(
        matches!(result, Err(Error::Syntax { .. })),
        "expected Syntax, got {result:?}"
    );
}

/// Pins `<LOC id>text`: the id and the text are split, the text may hold
/// spaces and punctuation, and the value is not a plain string.
#[test]
fn loc_strings_split_into_id_and_text() {
    let bp = parsed("FooBlueprint { Desc = '<LOC xyz_0001>Invented Name, Mk. 2' }");
    assert_eq!(
        bp.body.get("Desc"),
        Some(&Value::Loc {
            id: "xyz_0001".into(),
            text: "Invented Name, Mk. 2".into(),
        })
    );
}

/// Pins numerals: a leading minus belongs to the literal, and an integer
/// literal equals its decimal form.
#[test]
fn negative_numbers_and_integer_decimal_equality() {
    let bp = parsed("FooBlueprint { A = -3, B = -0.5, C = 1, D = 1.0, E = 0.25 }");
    let get = |k: &str| bp.body.get(k).and_then(Value::as_f64);
    assert_eq!(get("A"), Some(-3.0));
    assert_eq!(get("B"), Some(-0.5));
    assert_eq!(bp.body.get("C"), bp.body.get("D"));
    assert_eq!(get("E"), Some(0.25));
}

/// Pins the literals `true`, `false`, and `nil`.
#[test]
fn booleans_and_nil() {
    let bp = parsed("FooBlueprint { A = true, B = false, C = nil }");
    assert_eq!(bp.body.get("A"), Some(&Value::Bool(true)));
    assert_eq!(bp.body.get("B"), Some(&Value::Bool(false)));
    assert_eq!(bp.body.get("C"), Some(&Value::Nil));
}

/// Pins the parenthesised constructor call: kept as a `Call` with its
/// arguments unevaluated and in order, whatever the spacing.
#[test]
fn parenthesised_calls_are_kept_structurally() {
    let bp = parsed("FooBlueprint { P = VECTOR3( 1, -2.5, 3 ), S = STRING('q'), E = VECTOR3() }");
    assert_eq!(
        bp.body.get("P"),
        Some(&Value::Call {
            name: "VECTOR3".into(),
            args: vec![num(1.0), num(-2.5), num(3.0)],
        })
    );
    assert_eq!(
        bp.body.get("S"),
        Some(&Value::Call {
            name: "STRING".into(),
            args: vec![text("q")],
        })
    );
    assert_eq!(
        bp.body.get("E"),
        Some(&Value::Call {
            name: "VECTOR3".into(),
            args: vec![],
        })
    );
}

/// Pins the brace-form constructor call: `Name { .. }` is a `Call` whose one
/// argument is the table, nested and keyed like any table.
#[test]
fn brace_form_calls_carry_the_table_as_their_one_argument() {
    let bp =
        parsed("FooBlueprint { Snd = Sound { Bank = 'b', Cue = 'c' }, Inner = Group { 1, 2 } }");
    assert_eq!(
        bp.body.get("Snd"),
        Some(&Value::Call {
            name: "Sound".into(),
            args: vec![Value::Table(vec![
                (Key::Name("Bank".into()), text("b")),
                (Key::Name("Cue".into()), text("c")),
            ])],
        })
    );
    assert!(matches!(bp.body.get("Inner"), Some(Value::Call { name, .. }) if name == "Group"));
}

/// Pins the bracketed keys: a bracketed string is a `Name`, a bracketed
/// number is an `Index`, so `['a b']` and `[7]` both parse.
#[test]
fn bracketed_keys_are_names_and_indexes() {
    let bp = parsed("FooBlueprint { ['odd name'] = 1, [7] = 2, plain = 3 }");
    assert_eq!(
        bp.body.entries(),
        &[
            (Key::Name("odd name".into()), num(1.0)),
            (Key::Index(7), num(2.0)),
            (Key::Name("plain".into()), num(3.0)),
        ]
    );
}

/// Pins error positions: an unclosed table on line 3 is a `Syntax` error
/// that reports a line within the file and a column of at least 1, and a
/// stray token on line 2 is reported on line 2.
#[test]
fn syntax_errors_report_line_and_column() {
    let stray = parse("FooBlueprint {\n  A = = 1,\n}");
    assert!(
        matches!(stray, Err(Error::Syntax { line: 2, column, .. }) if column >= 1),
        "expected Syntax on line 2, got {stray:?}"
    );
    let unclosed = parse("FooBlueprint {\n  A = 1,\n  B = 2,\n");
    assert!(
        matches!(unclosed, Err(Error::Syntax { line, column, .. }) if (3..=4).contains(&line) && column >= 1),
        "expected Syntax on line 3 or 4, got {unclosed:?}"
    );
}

/// Pins the subset boundary: a function body is `Unsupported`, names the
/// construct, and reports the line it starts on; it is not a syntax error.
#[test]
fn function_is_unsupported_and_named() {
    let result = parse("FooBlueprint {\n  A = function() return 1 end,\n}");
    assert!(
        matches!(&result, Err(Error::Unsupported { line: 2, construct }) if construct.contains("function")),
        "expected Unsupported naming function at line 2, got {result:?}"
    );
}

/// Pins CRLF as whitespace: the same text with CRLF line ends parses to the
/// same blueprint as with LF, through `parse` and `parse_bytes`.
#[test]
fn crlf_input_parses_like_lf() {
    let lf = "# c\nFooBlueprint {\n  A = 1,\n  B = 'x',\n}\n";
    let crlf = lf.replace('\n', "\r\n");
    let from_lf = parsed(lf);
    assert_eq!(parsed(&crlf), from_lf);
    let from_bytes = parse_bytes(crlf.as_bytes());
    assert_eq!(from_bytes.as_ref().ok(), Some(&from_lf));
}

/// Pins the encoding: `parse_bytes` decodes ISO-8859-1, so byte 0xE9 in a
/// string is U+00E9 and bytes at or above 0x80 are not an error.
#[test]
fn parse_bytes_decodes_iso_8859_1() {
    let mut bytes = b"FooBlueprint { A = 'caf".to_vec();
    bytes.push(0xE9);
    bytes.extend_from_slice(b"' }");
    let result = parse_bytes(&bytes);
    assert_eq!(
        result
            .as_ref()
            .ok()
            .and_then(|bp| bp.body.get("A"))
            .and_then(Value::as_str),
        Some("caf\u{e9}"),
        "got {result:?}"
    );
}

/// Pins `parse_script`: top-level `name = value` assignments come back in
/// order, each value any data value, with comments between them.
#[test]
fn script_assignments_come_back_in_order() {
    let script = scripted("First = { A = 1 }\n-- between\nSecond = STRING('s')\nThird = 4\n");
    let names: Vec<&str> = script.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(names, ["First", "Second", "Third"]);
    assert_eq!(script[2].1, num(4.0));
    assert!(matches!(&script[1].1, Value::Call { name, .. } if name == "STRING"));
}

/// Builds a save-file text with the markers table `body` inside
/// `Scenario.MasterChain['_MASTERCHAIN_']`.
fn save_text(markers_body: &str) -> String {
    format!(
        "Scenario = {{\n  MasterChain = {{\n    ['_MASTERCHAIN_'] = {{\n      Markers = {{\n{markers_body}\n      }},\n    }},\n  }},\n}}\n"
    )
}

/// Pins `markers`: an entry with a `STRING` type and a `VECTOR3` position is
/// a marker (the numbers cast to f32); an entry lacking either, or with a
/// `type` that is not a `STRING` call, is skipped without an error.
#[test]
fn markers_extracts_complete_entries_and_skips_the_rest() {
    let script = scripted(&save_text(
        "['Alpha 01'] = { type = STRING( 'Mass' ), position = VECTOR3( 1.5, 2, -3 ), size = FLOAT( 1 ) },\n\
         ['Beta'] = { type = STRING( 'Blank' ) },\n\
         ['Gamma'] = { position = VECTOR3( 0, 0, 0 ) },\n\
         ['Delta'] = { type = 'Mass', position = VECTOR3( 0, 0, 0 ) },\n\
         ['Eps'] = { type = STRING( 'Weather' ), position = VECTOR3( 9, 8, 7 ) },",
    ));
    let found = markers(&script);
    assert_eq!(
        found,
        Ok(vec![
            Marker {
                name: "Alpha 01".into(),
                kind: "Mass".into(),
                position: [1.5, 2.0, -3.0],
            },
            Marker {
                name: "Eps".into(),
                kind: "Weather".into(),
                position: [9.0, 8.0, 7.0],
            },
        ])
    );
}

/// Pins `markers` on a save with an empty Markers table: no markers and no
/// error.
#[test]
fn markers_of_an_empty_table_is_empty() {
    let script = scripted(&save_text(""));
    assert_eq!(markers(&script), Ok(vec![]));
}
