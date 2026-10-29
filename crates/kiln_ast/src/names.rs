//! Declaration names (spec/ast.md §3).
//!
//! A [`DeclName`] is one `u32`: a 3-bit tag (bits 29..=31) and a 29-bit
//! payload. Names whose payload is not a small scalar (constructor,
//! destructor, conversion and deduction-guide names) index the interned
//! `NameTable`, so `DeclName` equality is `u32` equality and
//! `(context, DeclName)` is an 8-byte hashable lookup key.

use std::fmt;

use kiln_source::fx::FxHashMap;
use kiln_source::symbol::{Interner, Symbol};

use crate::ids::{DeclId, QualType, TypeId};
use crate::mem::{HeapSize, MemReport, MemRow};

const TAG_SHIFT: u32 = 29;
const PAYLOAD_MASK: u32 = (1 << TAG_SHIFT) - 1;

/// The kind of a [`DeclName`] (its tag).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[repr(u8)]
pub enum DeclNameKind {
    /// An identifier, or the empty name (unnamed entities).
    Identifier = 0,
    /// `operator@`.
    Operator = 1,
    /// A constructor name.
    Constructor = 2,
    /// A destructor name.
    Destructor = 3,
    /// A conversion-function name.
    Conversion = 4,
    /// A literal operator `operator""_x`.
    LiteralOperator = 5,
    /// A deduction-guide name.
    DeductionGuide = 6,
}

/// A declaration name (spec/ast.md §3), 4 bytes.
///
/// Encoding: identifiers store `Symbol::index() + 1` so that the all-zero
/// word is [`DeclName::EMPTY`] (symbol 0 is an ordinary identifier);
/// operators store their [`OverloadedOperator`]; literal operators the
/// suffix's `Symbol::index()`; the other kinds an index into
/// `NameTable`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct DeclName(u32);

impl DeclName {
    /// The empty name (unnamed parameters, anonymous classes, ...).
    pub const EMPTY: DeclName = DeclName(0);

    const fn pack(kind: DeclNameKind, payload: u32) -> DeclName {
        assert!(payload <= PAYLOAD_MASK, "DeclName payload exceeds 29 bits");
        DeclName(((kind as u32) << TAG_SHIFT) | payload)
    }

    /// The name of identifier `sym`.
    ///
    /// # Panics
    /// If `sym.index() >= 2^29 − 1` (more distinct identifiers than a
    /// `DeclName` can encode; ~5e8).
    pub const fn ident(sym: Symbol) -> DeclName {
        Self::pack(DeclNameKind::Identifier, sym.index() + 1)
    }

    /// `operator op`.
    pub const fn operator(op: OverloadedOperator) -> DeclName {
        Self::pack(DeclNameKind::Operator, op as u32)
    }

    /// The literal operator with suffix `sym` (`operator""_km`).
    pub const fn literal_operator(sym: Symbol) -> DeclName {
        Self::pack(DeclNameKind::LiteralOperator, sym.index())
    }

    /// The tag.
    pub const fn kind(self) -> DeclNameKind {
        match self.0 >> TAG_SHIFT {
            0 => DeclNameKind::Identifier,
            1 => DeclNameKind::Operator,
            2 => DeclNameKind::Constructor,
            3 => DeclNameKind::Destructor,
            4 => DeclNameKind::Conversion,
            5 => DeclNameKind::LiteralOperator,
            6 => DeclNameKind::DeductionGuide,
            _ => unreachable!(),
        }
    }

    /// The 29-bit payload.
    pub const fn payload(self) -> u32 {
        self.0 & PAYLOAD_MASK
    }

    /// True for [`DeclName::EMPTY`].
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// The identifier, if this is a non-empty identifier name.
    pub const fn as_ident(self) -> Option<Symbol> {
        if self.0 != 0 && self.0 >> TAG_SHIFT == 0 {
            Some(Symbol::from_index(self.0 - 1))
        } else {
            None
        }
    }

    /// The operator, if this is an `operator@` name.
    pub fn as_operator(self) -> Option<OverloadedOperator> {
        if self.kind() == DeclNameKind::Operator {
            OverloadedOperator::from_u8(self.payload() as u8)
        } else {
            None
        }
    }

    /// The raw word.
    pub const fn to_raw(self) -> u32 {
        self.0
    }

    /// The name with raw word `raw` (tag 7 is reserved: `None`).
    pub const fn from_raw(raw: u32) -> Option<DeclName> {
        if raw >> TAG_SHIFT == 7 { None } else { Some(DeclName(raw)) }
    }
}

impl fmt::Debug for DeclName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_empty() {
            return f.write_str("DeclName(empty)");
        }
        write!(f, "DeclName({:?} {})", self.kind(), self.payload())
    }
}

/// A decoded [`DeclName`] (see [`NameTable::view`]).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DeclNameView {
    /// The empty name.
    Empty,
    /// An identifier.
    Identifier(Symbol),
    /// `operator@`.
    Operator(OverloadedOperator),
    /// Constructor of the class whose canonical type is given.
    Constructor(TypeId),
    /// Destructor of the class whose canonical type is given.
    Destructor(TypeId),
    /// Conversion function to the given canonical type.
    Conversion(QualType),
    /// Literal operator with the given suffix.
    LiteralOperator(Symbol),
    /// Deduction guide of the given template.
    DeductionGuide(DeclId),
}

/// Overloadable operators ([over.oper]), clang's `OverloadedOperatorKind`
/// order.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[repr(u8)]
#[allow(missing_docs, reason = "each variant is named after its spelling")]
pub enum OverloadedOperator {
    New = 1,
    Delete,
    ArrayNew,
    ArrayDelete,
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Caret,
    Amp,
    Pipe,
    Tilde,
    Exclaim,
    Equal,
    Less,
    Greater,
    PlusEqual,
    MinusEqual,
    StarEqual,
    SlashEqual,
    PercentEqual,
    CaretEqual,
    AmpEqual,
    PipeEqual,
    LessLess,
    GreaterGreater,
    LessLessEqual,
    GreaterGreaterEqual,
    EqualEqual,
    ExclaimEqual,
    LessEqual,
    GreaterEqual,
    Spaceship,
    AmpAmp,
    PipePipe,
    PlusPlus,
    MinusMinus,
    Comma,
    ArrowStar,
    Arrow,
    Call,
    Subscript,
    Coawait,
}

impl OverloadedOperator {
    const ALL: [OverloadedOperator; 44] = {
        use OverloadedOperator as O;
        [
            O::New,
            O::Delete,
            O::ArrayNew,
            O::ArrayDelete,
            O::Plus,
            O::Minus,
            O::Star,
            O::Slash,
            O::Percent,
            O::Caret,
            O::Amp,
            O::Pipe,
            O::Tilde,
            O::Exclaim,
            O::Equal,
            O::Less,
            O::Greater,
            O::PlusEqual,
            O::MinusEqual,
            O::StarEqual,
            O::SlashEqual,
            O::PercentEqual,
            O::CaretEqual,
            O::AmpEqual,
            O::PipeEqual,
            O::LessLess,
            O::GreaterGreater,
            O::LessLessEqual,
            O::GreaterGreaterEqual,
            O::EqualEqual,
            O::ExclaimEqual,
            O::LessEqual,
            O::GreaterEqual,
            O::Spaceship,
            O::AmpAmp,
            O::PipePipe,
            O::PlusPlus,
            O::MinusMinus,
            O::Comma,
            O::ArrowStar,
            O::Arrow,
            O::Call,
            O::Subscript,
            O::Coawait,
        ]
    };

    /// The operator with discriminant `v`.
    pub fn from_u8(v: u8) -> Option<Self> {
        Self::ALL.get(usize::from(v).wrapping_sub(1)).copied()
    }

    /// The spelling after `operator` (`+`, `new[]`, `()`, `co_await`).
    pub const fn spelling(self) -> &'static str {
        use OverloadedOperator as O;
        match self {
            O::New => "new",
            O::Delete => "delete",
            O::ArrayNew => "new[]",
            O::ArrayDelete => "delete[]",
            O::Plus => "+",
            O::Minus => "-",
            O::Star => "*",
            O::Slash => "/",
            O::Percent => "%",
            O::Caret => "^",
            O::Amp => "&",
            O::Pipe => "|",
            O::Tilde => "~",
            O::Exclaim => "!",
            O::Equal => "=",
            O::Less => "<",
            O::Greater => ">",
            O::PlusEqual => "+=",
            O::MinusEqual => "-=",
            O::StarEqual => "*=",
            O::SlashEqual => "/=",
            O::PercentEqual => "%=",
            O::CaretEqual => "^=",
            O::AmpEqual => "&=",
            O::PipeEqual => "|=",
            O::LessLess => "<<",
            O::GreaterGreater => ">>",
            O::LessLessEqual => "<<=",
            O::GreaterGreaterEqual => ">>=",
            O::EqualEqual => "==",
            O::ExclaimEqual => "!=",
            O::LessEqual => "<=",
            O::GreaterEqual => ">=",
            O::Spaceship => "<=>",
            O::AmpAmp => "&&",
            O::PipePipe => "||",
            O::PlusPlus => "++",
            O::MinusMinus => "--",
            O::Comma => ",",
            O::ArrowStar => "->*",
            O::Arrow => "->",
            O::Call => "()",
            O::Subscript => "[]",
            O::Coawait => "co_await",
        }
    }
}

/// Interned operands of constructor, destructor, conversion and
/// deduction-guide names (spec/ast.md §3 `names.special`).
#[derive(Debug, Default)]
pub struct NameTable {
    special: Vec<(DeclNameKind, u32)>,
    index: FxHashMap<(DeclNameKind, u32), u32>,
}

impl NameTable {
    /// An empty table.
    pub fn new() -> Self {
        Self::default()
    }

    fn intern(&mut self, kind: DeclNameKind, operand: u32) -> DeclName {
        let next = self.special.len() as u32;
        let idx = *self.index.entry((kind, operand)).or_insert(next);
        if idx == next {
            self.special.push((kind, operand));
        }
        DeclName::pack(kind, idx)
    }

    /// The constructor name of the class with canonical type `class`.
    pub fn constructor(&mut self, class: TypeId) -> DeclName {
        self.intern(DeclNameKind::Constructor, class.index())
    }

    /// The destructor name of the class with canonical type `class`.
    pub fn destructor(&mut self, class: TypeId) -> DeclName {
        self.intern(DeclNameKind::Destructor, class.index())
    }

    /// The constructor name of `class` if it was ever interned (a read:
    /// no class without a constructor name has constructors).
    pub fn find_constructor(&self, class: TypeId) -> Option<DeclName> {
        self.index
            .get(&(DeclNameKind::Constructor, class.index()))
            .map(|&i| DeclName::pack(DeclNameKind::Constructor, i))
    }

    /// The destructor name of `class` if it was ever interned.
    pub fn find_destructor(&self, class: TypeId) -> Option<DeclName> {
        self.index
            .get(&(DeclNameKind::Destructor, class.index()))
            .map(|&i| DeclName::pack(DeclNameKind::Destructor, i))
    }

    /// The name of a conversion function to canonical type `target`.
    pub fn conversion(&mut self, target: QualType) -> DeclName {
        self.intern(DeclNameKind::Conversion, target.to_raw())
    }

    /// The deduction-guide name of template `template`.
    pub fn deduction_guide(&mut self, template: DeclId) -> DeclName {
        self.intern(DeclNameKind::DeductionGuide, template.index())
    }

    /// Decodes `name`.
    ///
    /// # Panics
    /// If `name` indexes a special name not created by this table.
    pub fn view(&self, name: DeclName) -> DeclNameView {
        let p = name.payload();
        let special = || self.special[p as usize].1;
        match name.kind() {
            DeclNameKind::Identifier => {
                name.as_ident().map_or(DeclNameView::Empty, DeclNameView::Identifier)
            }
            DeclNameKind::Operator => DeclNameView::Operator(
                OverloadedOperator::from_u8(p as u8).expect("DeclName: invalid operator payload"),
            ),
            DeclNameKind::Constructor => DeclNameView::Constructor(TypeId::new(special())),
            DeclNameKind::Destructor => DeclNameView::Destructor(TypeId::new(special())),
            DeclNameKind::Conversion => DeclNameView::Conversion(
                QualType::from_raw(special()).expect("DeclName: invalid conversion operand"),
            ),
            DeclNameKind::LiteralOperator => DeclNameView::LiteralOperator(Symbol::from_index(p)),
            DeclNameKind::DeductionGuide => DeclNameView::DeductionGuide(DeclId::new(special())),
        }
    }

    /// The spelling of identifier-like names (identifiers, `operator@`,
    /// `operator""_x`); `None` for names that need a type printer
    /// (constructors, destructors, conversions, deduction guides).
    pub fn simple_spelling(&self, name: DeclName, symbols: &Interner) -> Option<String> {
        match self.view(name) {
            DeclNameView::Empty => Some(String::new()),
            DeclNameView::Identifier(s) => Some(symbols.as_str(s).to_owned()),
            DeclNameView::Operator(op) => {
                let sp = op.spelling();
                let sep = if sp.as_bytes()[0].is_ascii_alphabetic() { " " } else { "" };
                Some(format!("operator{sep}{sp}"))
            }
            DeclNameView::LiteralOperator(s) => Some(format!("operator\"\"{}", symbols.as_str(s))),
            _ => None,
        }
    }
}

impl HeapSize for NameTable {
    fn heap_bytes(&self) -> MemReport {
        let mut r = MemReport::new();
        r.push(MemRow::of_vec("special", &self.special));
        r.push(MemRow::of_map("special_index", &self.index));
        r
    }
}

#[cfg(test)]
mod tests {
    use super::{DeclName, DeclNameKind, DeclNameView, NameTable, OverloadedOperator};
    use crate::ids::{DeclId, QualType, TypeId};
    use kiln_source::symbol::Interner;

    #[test]
    fn encodings_roundtrip() {
        let i = Interner::new();
        let s0 = i.intern("first");
        let n = DeclName::ident(s0);
        assert!(!n.is_empty());
        assert_eq!(n.as_ident(), Some(s0));
        assert_ne!(n, DeclName::EMPTY);
        let mut t = NameTable::new();
        let ctor = t.constructor(TypeId::new(70));
        assert_eq!(t.constructor(TypeId::new(70)), ctor, "special names are interned");
        assert_ne!(t.destructor(TypeId::new(70)), ctor);
        assert_eq!(ctor.kind(), DeclNameKind::Constructor);
        assert_eq!(t.view(ctor), DeclNameView::Constructor(TypeId::new(70)));
        let conv = t.conversion(QualType::new(TypeId::new(12), 1));
        assert_eq!(t.view(conv), DeclNameView::Conversion(QualType::new(TypeId::new(12), 1)));
        let g = t.deduction_guide(DeclId::new(4));
        assert_eq!(t.view(g), DeclNameView::DeductionGuide(DeclId::new(4)));
        let op = DeclName::operator(OverloadedOperator::ArrayNew);
        assert_eq!(op.as_operator(), Some(OverloadedOperator::ArrayNew));
        assert_eq!(t.simple_spelling(op, &i).as_deref(), Some("operator new[]"));
        let pl = DeclName::operator(OverloadedOperator::PlusEqual);
        assert_eq!(t.simple_spelling(pl, &i).as_deref(), Some("operator+="));
        let lit = DeclName::literal_operator(i.intern("_km"));
        assert_eq!(t.simple_spelling(lit, &i).as_deref(), Some("operator\"\"_km"));
        assert_eq!(t.view(DeclName::EMPTY), DeclNameView::Empty);
        assert_eq!(DeclName::from_raw(7 << 29), None);
        for v in 1..=44u8 {
            assert_eq!(OverloadedOperator::from_u8(v).map(|o| o as u8), Some(v));
        }
        assert_eq!(OverloadedOperator::from_u8(0), None);
        assert_eq!(OverloadedOperator::from_u8(45), None);
    }
}
