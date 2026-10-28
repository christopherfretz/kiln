//! Macro definitions and the macro table ([cpp.replace]).

use std::cell::Cell;
use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hasher};
use std::rc::Rc;

use kiln_lex::{Symbol, Token};
use kiln_source::{Loc, Span};

use crate::builtin::Builtin;
use crate::tok::PpTok;

/// A fast hasher for `Symbol` keys (a multiplicative hash of the `u32`):
/// `Symbol`s are dense small integers, so `SipHash`'s `DoS` resistance buys
/// nothing here.
#[derive(Default, Clone, Copy, Debug)]
pub(crate) struct SymHasher(u64);

impl Hasher for SymHasher {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 = (self.0.rotate_left(5) ^ u64::from(b)).wrapping_mul(0x51_7c_c1_b7_27_22_0a_95);
        }
    }

    fn write_u32(&mut self, n: u32) {
        self.0 = (self.0.rotate_left(5) ^ u64::from(n)).wrapping_mul(0x51_7c_c1_b7_27_22_0a_95);
    }
}

/// `HashMap` keyed by `Symbol` with [`SymHasher`].
pub(crate) type SymMap<V> = HashMap<Symbol, V, BuildHasherDefault<SymHasher>>;

/// One macro definition.
#[derive(Debug)]
pub struct MacroDef {
    pub(crate) name: Symbol,
    pub(crate) name_span: Span,
    pub(crate) params: Box<[Symbol]>,
    pub(crate) function_like: bool,
    /// `...` or GNU `name...`: the last parameter collects the rest.
    pub(crate) variadic: bool,
    /// The variadic parameter is named (GNU `args...`).
    pub(crate) named_variadic: bool,
    pub(crate) body: Box<[PpTok]>,
    /// Byte offsets `(lo, hi)` of each body token relative to `def_start`
    /// (to map body tokens into an expansion entry without a source
    /// manager lookup per token).
    pub(crate) rel: Box<[(u32, u32)]>,
    /// Start of the definition range: the macro-name token.
    pub(crate) def_start: Loc,
    /// Length of the definition range (name through the last body token).
    pub(crate) def_len: u32,
    pub(crate) builtin: Option<Builtin>,
    /// False while the macro is being expanded ([cpp.rescan]/2).
    pub(crate) enabled: Cell<bool>,
    pub(crate) used: Cell<bool>,
    /// clang `isAllowRedefinitionsWithoutWarning`: set by
    /// `#pragma push_macro` so a later redefinition does not warn.
    pub(crate) allow_redef: Cell<bool>,
}

impl MacroDef {
    /// The macro's name.
    pub fn name(&self) -> Symbol {
        self.name
    }

    /// The name token in the `#define` (invalid for builtins).
    pub fn span(&self) -> Span {
        self.name_span
    }

    /// Function-like (`#define F(...)`).
    pub fn is_function_like(&self) -> bool {
        self.function_like
    }

    /// Takes variable arguments.
    pub fn is_variadic(&self) -> bool {
        self.variadic
    }

    /// Parameter names (`__VA_ARGS__` for an unnamed `...`).
    pub fn params(&self) -> &[Symbol] {
        &self.params
    }

    /// The replacement list as written (locations in the `#define`).
    pub fn body(&self) -> impl ExactSizeIterator<Item = Token> + '_ {
        self.body.iter().map(|t| t.tok)
    }

    /// A builtin macro (`__LINE__`, `__has_include`, `_Pragma`, ...).
    pub fn is_builtin(&self) -> bool {
        self.builtin.is_some()
    }

    /// Has been expanded at least once.
    pub fn is_used(&self) -> bool {
        self.used.get()
    }

    pub(crate) fn param_index(&self, s: Symbol) -> Option<usize> {
        self.params.iter().position(|&p| p == s)
    }
}

/// The macro table: name to current definition.
#[derive(Debug, Default)]
pub struct MacroTable {
    pub(crate) map: SymMap<Rc<MacroDef>>,
}

impl MacroTable {
    /// The current definition of `name`.
    pub fn get(&self, name: Symbol) -> Option<&MacroDef> {
        self.map.get(&name).map(|d| &**d)
    }

    /// True if `name` is defined (including builtins).
    pub fn is_defined(&self, name: Symbol) -> bool {
        self.map.contains_key(&name)
    }

    /// Number of defined macros (including builtins).
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// True if no macro is defined.
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// All definitions, in unspecified order.
    pub fn iter(&self) -> impl Iterator<Item = (Symbol, &MacroDef)> + '_ {
        self.map.iter().map(|(&k, v)| (k, &**v))
    }

    pub(crate) fn get_rc(&self, name: Symbol) -> Option<&Rc<MacroDef>> {
        self.map.get(&name)
    }
}
