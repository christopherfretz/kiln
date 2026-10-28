//! Constant values: clang's text-dump `value:` trees (and `ConstantExpr`
//! JSON values) re-printed the way kiln prints `value=`
//! (`spec/ast.md` §9.3, `kiln_ast::consts::print` in `Dump` style).
//!
//! clang's tree is untyped (`Int 1` for `true`, `Float 1.500000e+00`), so
//! printing walks the declared type alongside it. A `Float` is printed by
//! clang with only 7 significant digits; its exact value is recovered from
//! the literals in the initializer, and a value that cannot be recovered
//! unambiguously is an error.

use std::collections::{HashMap, HashSet};

use super::side::VNode;

/// Aggregates nested this deep print as `{...}` (kiln's default).
const MAX_DEPTH: u32 = 10;
/// Arrays and records print at most this many elements (kiln's default).
const MAX_ELEMS: usize = 32;

/// A field of a record, as the JSON declares it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Field {
    /// `None` for an unnamed bit-field.
    pub name: Option<String>,
    /// The declared type (clang's `qualType`).
    pub ty: String,
}

/// A class or union definition.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Record {
    pub union: bool,
    pub fields: Vec<Field>,
}

/// A name that may be declared more than once with different meanings
/// (local classes and typedefs in different functions).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Named<T> {
    One(T),
    Ambiguous,
}

/// The types of the translation unit, by unqualified name.
#[derive(Debug, Default)]
pub struct Types {
    pub records: HashMap<String, Named<Record>>,
    pub aliases: HashMap<String, Named<String>>,
    pub enums: HashSet<String>,
}

fn insert<T: PartialEq>(map: &mut HashMap<String, Named<T>>, name: &str, v: T) {
    match map.get(name) {
        None => {
            map.insert(name.to_owned(), Named::One(v));
        }
        Some(Named::One(old)) if *old == v => {}
        Some(_) => {
            map.insert(name.to_owned(), Named::Ambiguous);
        }
    }
}

/// What a value of some type looks like.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Shape {
    Bool,
    /// Integers, characters and enumerations (all print in decimal).
    Int,
    F32,
    F64,
    NullPtr,
    /// Pointers and references.
    Pointer,
    Array {
        len: u64,
        elem: String,
    },
    Record(String),
}

const INT_TYPES: &[&str] = &[
    "char",
    "signed char",
    "unsigned char",
    "char8_t",
    "char16_t",
    "char32_t",
    "wchar_t",
    "short",
    "unsigned short",
    "int",
    "unsigned int",
    "long",
    "unsigned long",
    "long long",
    "unsigned long long",
    "__int128",
    "unsigned __int128",
];

impl Types {
    pub fn add_record(&mut self, name: &str, r: Record) {
        insert(&mut self.records, name, r);
    }

    pub fn add_alias(&mut self, name: &str, ty: &str) {
        insert(&mut self.aliases, name, ty.to_owned());
    }

    /// The shape of the type clang prints as `ty`.
    pub fn shape(&self, ty: &str) -> Result<Shape, String> {
        let mut t = ty.trim().to_owned();
        // Bounded: each round removes one alias; 64 guards a cycle.
        for _ in 0..64 {
            let mut s = t.as_str();
            loop {
                let before = s;
                for q in ["const ", "volatile "] {
                    s = s.strip_prefix(q).unwrap_or(s);
                }
                if s == before {
                    break;
                }
            }
            if s.contains('(') {
                if s.contains("(*") || s.contains("(&") {
                    return Ok(Shape::Pointer);
                }
                return Err(format!("clang-ast: no value shape for function type `{ty}`"));
            }
            let bare = s.trim_end_matches(|c: char| c.is_ascii_lowercase() || c == ' ');
            if bare.ends_with('*') || bare.ends_with('&') {
                return Ok(Shape::Pointer);
            }
            if s.ends_with(']') {
                let open = s.find('[').unwrap_or(0);
                let close = s[open..].find(']').map_or(s.len(), |c| open + c);
                let len = s[open + 1..close]
                    .parse::<u64>()
                    .map_err(|_| format!("clang-ast: no value shape for array type `{ty}`"))?;
                let elem = format!("{}{}", s[..open].trim_end(), &s[close + 1..]);
                return Ok(Shape::Array { len, elem });
            }
            for tag in ["struct ", "class ", "union ", "enum "] {
                s = s.strip_prefix(tag).unwrap_or(s);
            }
            match s {
                "bool" => return Ok(Shape::Bool),
                "float" => return Ok(Shape::F32),
                "double" => return Ok(Shape::F64),
                "std::nullptr_t" | "nullptr_t" | "decltype(nullptr)" => return Ok(Shape::NullPtr),
                _ if INT_TYPES.contains(&s) => return Ok(Shape::Int),
                _ => {}
            }
            if s.contains('<') {
                return Err(format!("clang-ast: no value shape for template type `{ty}`"));
            }
            let name = s.rsplit("::").next().unwrap_or(s);
            if let Some(r) = self.records.get(name) {
                return match r {
                    Named::One(_) => Ok(Shape::Record(name.to_owned())),
                    Named::Ambiguous => Err(format!(
                        "clang-ast: class name `{name}` is defined more than once; \
                         its values cannot be printed"
                    )),
                };
            }
            if self.enums.contains(name) {
                return Ok(Shape::Int);
            }
            match self.aliases.get(name) {
                Some(Named::One(a)) => t = a.clone(),
                Some(Named::Ambiguous) => {
                    return Err(format!(
                        "clang-ast: type alias `{name}` is declared more than once with \
                         different types; its values cannot be printed"
                    ));
                }
                None => return Err(format!("clang-ast: no value shape for type `{ty}`")),
            }
        }
        Err(format!("clang-ast: type alias cycle resolving `{ty}`"))
    }

    fn record(&self, name: &str) -> Result<&Record, String> {
        match self.records.get(name) {
            Some(Named::One(r)) => Ok(r),
            _ => Err(format!("clang-ast: no unique definition of class `{name}`")),
        }
    }
}

/// A value as clang's text dump prints it.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Int(String),
    Float(String),
    /// clang's `LValue`: a pointer or reference value (printed `LValue <todo>`).
    Address,
    /// The value of an unnamed bit-field.
    Absent,
    Struct(Vec<Value>),
    Array {
        size: u64,
        elems: Vec<Value>,
        filler: Option<(u64, Box<Value>)>,
    },
    Union(Option<(String, Box<Value>)>),
}

fn bad(what: &str) -> String {
    format!("clang-ast: unsupported value in clang's text dump: `{what}`")
}

/// Parses a one-line ("simple") value.
fn simple(s: &str) -> Result<Value, String> {
    let s = s.trim();
    if let Some(n) = s.strip_prefix("Int ") {
        return Ok(Value::Int(n.to_owned()));
    }
    if let Some(f) = s.strip_prefix("Float ") {
        return Ok(Value::Float(f.to_owned()));
    }
    if s == "LValue" || s.starts_with("LValue ") {
        return Ok(Value::Address);
    }
    if s == "None" {
        return Ok(Value::Absent);
    }
    if s == "Union" {
        return Ok(Value::Union(None));
    }
    if let Some(rest) = s.strip_prefix("Union .") {
        let (name, v) = rest.split_once(' ').ok_or_else(|| bad(s))?;
        return Ok(Value::Union(Some((name.to_owned(), Box::new(simple(v)?)))));
    }
    Err(bad(s))
}

/// Parses the values on a `fields:`/`elements:` line, or the one value of a
/// `field:`/`element:` line with its own children.
fn items(n: &VNode, text: &str, out: &mut Vec<Value>) -> Result<(), String> {
    if n.children.is_empty() {
        for part in text.split(", ") {
            out.push(simple(part)?);
        }
        Ok(())
    } else {
        out.push(parse(&VNode { label: String::new(), text: text.to_owned(), ..n.clone() })?);
        Ok(())
    }
}

/// Parses a `value:` subtree (or one of its non-simple children).
pub fn parse(n: &VNode) -> Result<Value, String> {
    let t = n.text.as_str();
    if t == "Struct" {
        let mut fields = Vec::new();
        for c in &n.children {
            match c.label.as_str() {
                "field" | "fields" => items(c, &c.text, &mut fields)?,
                _ => return Err(bad(&format!("Struct child {}: {}", c.label, c.text))),
            }
        }
        return Ok(Value::Struct(fields));
    }
    if let Some(size) = t.strip_prefix("Array size=") {
        let size = size.parse::<u64>().map_err(|_| bad(t))?;
        let mut elems = Vec::new();
        let mut filler = None;
        for c in &n.children {
            match c.label.as_str() {
                "element" | "elements" => items(c, &c.text, &mut elems)?,
                "filler" => {
                    let (count, v) = c.text.split_once(" x ").ok_or_else(|| bad(&c.text))?;
                    let count = count.parse::<u64>().map_err(|_| bad(&c.text))?;
                    let mut one = Vec::new();
                    items(c, v, &mut one)?;
                    let v = one.pop().filter(|_| one.is_empty()).ok_or_else(|| bad(&c.text))?;
                    filler = Some((count, Box::new(v)));
                }
                _ => return Err(bad(&format!("Array child {}: {}", c.label, c.text))),
            }
        }
        return Ok(Value::Array { size, elems, filler });
    }
    if let Some(name) = t.strip_prefix("Union .")
        && !name.contains(' ')
    {
        let [child] = n.children.as_slice() else {
            return Err(bad(t));
        };
        return Ok(Value::Union(Some((name.to_owned(), Box::new(parse(child)?)))));
    }
    if !n.children.is_empty() && !t.starts_with("LValue") {
        return Err(bad(t));
    }
    simple(t)
}

/// The literal values found in an initializer, for float recovery.
#[derive(Debug, Default)]
pub struct Literals {
    /// `FloatingLiteral` and `IntegerLiteral` value strings.
    pub values: Vec<String>,
}

/// C's `%e` for `v` (`1.500000e+00`).
#[allow(clippy::many_single_char_names)]
fn c_e(v: f64) -> String {
    let s = format!("{v:.6e}");
    match s.split_once('e') {
        Some((m, e)) => {
            let (sign, digits) = match e.strip_prefix('-') {
                Some(d) => ('-', d),
                None => ('+', e),
            };
            format!("{m}e{sign}{digits:0>2}")
        }
        None => s.replace("NaN", "nan"),
    }
}

/// kiln's spelling of a float (`format!("{:e}")`, `nan`).
fn kiln_float<T: std::fmt::LowerExp>(v: T) -> String {
    let s = format!("{v:e}");
    if s == "NaN" { "nan".into() } else { s }
}

/// Recovers the exact value clang printed as `shown` from `lits`.
fn recover(shown: &str, single: bool, lits: &Literals) -> Result<String, String> {
    let mut found: Option<String> = None;
    // Zero is always a candidate: value-initialized elements and members
    // have no literal.
    for lit in lits.values.iter().map(String::as_str).chain(["0"]) {
        for neg in [false, true] {
            let text = if neg { format!("-{lit}") } else { lit.to_owned() };
            let (wide, spelled) = if single {
                match text.parse::<f32>() {
                    Ok(v) => (f64::from(v), kiln_float(v)),
                    Err(_) => continue,
                }
            } else {
                match text.parse::<f64>() {
                    Ok(v) => (v, kiln_float(v)),
                    Err(_) => continue,
                }
            };
            if c_e(wide) != shown {
                continue;
            }
            match &found {
                Some(f) if *f != spelled => {
                    return Err(format!(
                        "clang-ast: float value `{shown}` matches both {f} and {spelled}; \
                         cannot tell which it is"
                    ));
                }
                _ => found = Some(spelled),
            }
        }
    }
    found.ok_or_else(|| {
        format!(
            "clang-ast: cannot recover the exact float value clang printed as `{shown}` \
             (only zero and literal values in the initializer, possibly negated, are \
             recoverable)"
        )
    })
}

/// Prints `v`, a value of type `ty`, as kiln's `value=` does.
pub fn print(
    types: &Types,
    v: &Value,
    ty: &str,
    lits: &Literals,
    depth: u32,
    out: &mut String,
) -> Result<(), String> {
    let mismatch =
        || format!("clang-ast: value `{v:?}` does not fit its type `{ty}` (normalizer bug?)");
    match (types.shape(ty)?, v) {
        (Shape::Bool, Value::Int(n)) => match n.as_str() {
            "0" => out.push_str("false"),
            "1" => out.push_str("true"),
            _ => return Err(mismatch()),
        },
        (Shape::Int, Value::Int(n)) => {
            n.parse::<i128>().map_err(|_| mismatch())?;
            out.push_str(n);
        }
        (Shape::F32, Value::Float(f)) => out.push_str(&recover(f, true, lits)?),
        (Shape::F64, Value::Float(f)) => out.push_str(&recover(f, false, lits)?),
        (Shape::NullPtr, Value::Address | Value::Int(_)) => out.push_str("nullptr"),
        (Shape::Pointer, Value::Address) => {
            return Err(format!(
                "clang-ast: pointer or reference value of type `{ty}`: clang's dump \
                 (`LValue <todo>`) does not say what it points to"
            ));
        }
        (Shape::Array { len, elem }, Value::Array { size, elems, filler }) => {
            if len != *size {
                return Err(mismatch());
            }
            if depth >= MAX_DEPTH {
                out.push_str("{...}");
                return Ok(());
            }
            out.push('{');
            let shown = len.min(MAX_ELEMS as u64) as usize;
            for i in 0..shown {
                if i > 0 {
                    out.push_str(", ");
                }
                let e = match elems.get(i) {
                    Some(e) => e,
                    None => match filler {
                        Some((_, f)) => f,
                        None => return Err(mismatch()),
                    },
                };
                print(types, e, &elem, lits, depth + 1, out)?;
            }
            if (shown as u64) < len {
                out.push_str(if shown > 0 { ", ..." } else { "..." });
            }
            out.push('}');
        }
        (Shape::Record(name), Value::Struct(vals)) => {
            let r = types.record(&name)?;
            if r.union || r.fields.len() != vals.len() {
                return Err(mismatch());
            }
            if depth >= MAX_DEPTH {
                out.push_str("{...}");
                return Ok(());
            }
            out.push('{');
            let shown = r.fields.iter().zip(vals).filter(|(f, _)| f.name.is_some());
            for (n, (f, fv)) in shown.enumerate() {
                if n == MAX_ELEMS {
                    out.push_str(", ...");
                    break;
                }
                if n > 0 {
                    out.push_str(", ");
                }
                print(types, fv, &f.ty, lits, depth + 1, out)?;
            }
            out.push('}');
        }
        (Shape::Record(name), Value::Union(active)) => {
            let r = types.record(&name)?;
            if !r.union {
                return Err(mismatch());
            }
            if depth >= MAX_DEPTH {
                out.push_str("{...}");
                return Ok(());
            }
            out.push('{');
            if let Some((field, fv)) = active {
                let f = r
                    .fields
                    .iter()
                    .find(|f| f.name.as_deref() == Some(field))
                    .ok_or_else(mismatch)?;
                out.push('.');
                out.push_str(field);
                out.push_str(" = ");
                print(types, fv, &f.ty, lits, depth + 1, out)?;
            }
            out.push('}');
        }
        _ => return Err(mismatch()),
    }
    Ok(())
}

/// A top-level variable value: like [`print`], but a null pointer constant
/// initializer of pointer type prints `nullptr` (clang shows every pointer
/// as `LValue <todo>`; kiln shows null as `nullptr`).
pub fn print_var(
    types: &Types,
    v: &Value,
    ty: &str,
    lits: &Literals,
    null_init: bool,
) -> Result<String, String> {
    let mut out = String::new();
    if matches!(v, Value::Address) && null_init && types.shape(ty)? == Shape::Pointer {
        out.push_str("nullptr");
    } else {
        print(types, v, ty, lits, 0, &mut out)?;
    }
    Ok(out)
}

/// A `ConstantExpr` JSON `value` (`APValue::getAsString`) as kiln prints
/// it.
pub fn constant(types: &Types, value: &str, ty: &str) -> Result<String, String> {
    let ok = match types.shape(ty)? {
        Shape::Bool => value == "true" || value == "false",
        Shape::Int => value.parse::<i128>().is_ok(),
        Shape::NullPtr | Shape::Pointer => value == "nullptr",
        Shape::F32 | Shape::F64 => {
            return Err(format!(
                "clang-ast: floating `ConstantExpr` value `{value}`: clang's JSON rounds it; \
                 not recoverable"
            ));
        }
        Shape::Array { .. } | Shape::Record(_) => false,
    };
    if ok {
        Ok(value.to_owned())
    } else {
        Err(format!("clang-ast: unsupported `ConstantExpr` value `{value}` of type `{ty}`"))
    }
}
