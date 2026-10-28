//! Function, return and parameter attributes (`spec/llvm.md` §5).
//!
//! An attribute set is a sorted, deduplicated list of [`Attr`]s interned in
//! the module ([`crate::Module::attr_set`]); equal sets share an
//! [`AttrSetId`]. The sort order matches LLVM 19's (enum attributes by
//! `TableGen` record name, then type attributes, then integer attributes, then
//! string attributes by key), so printed sets look like clang's.

use std::collections::HashMap;
use std::fmt;

use crate::types::{TypeId, TypeTable};
use crate::value::AttrSetId;

macro_rules! attr_kinds {
    ($($v:ident = $s:literal,)*) => {
        /// Attributes without an argument, in LLVM 19's sort order.
        #[derive(Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
        #[allow(missing_docs)]
        pub enum AttrKind { $($v,)* }
        impl AttrKind {
            /// The textual IR spelling.
            pub fn name(self) -> &'static str {
                match self { $(AttrKind::$v => $s,)* }
            }
        }
    };
}

attr_kinds! {
    AllocAlign = "allocalign",
    AllocatedPointer = "allocptr",
    AlwaysInline = "alwaysinline",
    Builtin = "builtin",
    Cold = "cold",
    Convergent = "convergent",
    CoroDestroyOnlyWhenComplete = "coro_only_destroy_when_complete",
    DeadOnUnwind = "dead_on_unwind",
    DisableSanitizerInstrumentation = "disable_sanitizer_instrumentation",
    FnRetThunkExtern = "fn_ret_thunk_extern",
    Hot = "hot",
    HybridPatchable = "hybrid_patchable",
    ImmArg = "immarg",
    InReg = "inreg",
    InlineHint = "inlinehint",
    JumpTable = "jumptable",
    MinSize = "minsize",
    MustProgress = "mustprogress",
    Naked = "naked",
    Nest = "nest",
    NoAlias = "noalias",
    NoBuiltin = "nobuiltin",
    NoCallback = "nocallback",
    NoCapture = "nocapture",
    NoCfCheck = "nocf_check",
    NoDuplicate = "noduplicate",
    NoFree = "nofree",
    NoImplicitFloat = "noimplicitfloat",
    NoInline = "noinline",
    NoMerge = "nomerge",
    NoProfile = "noprofile",
    NoRecurse = "norecurse",
    NoRedZone = "noredzone",
    NoReturn = "noreturn",
    NoSanitizeBounds = "nosanitize_bounds",
    NoSanitizeCoverage = "nosanitize_coverage",
    NoSync = "nosync",
    NoUndef = "noundef",
    NoUnwind = "nounwind",
    NonLazyBind = "nonlazybind",
    NonNull = "nonnull",
    NullPointerIsValid = "null_pointer_is_valid",
    OptForFuzzing = "optforfuzzing",
    OptimizeForDebugging = "optdebug",
    OptimizeForSize = "optsize",
    OptimizeNone = "optnone",
    PresplitCoroutine = "presplitcoroutine",
    ReadNone = "readnone",
    ReadOnly = "readonly",
    Returned = "returned",
    ReturnsTwice = "returns_twice",
    SExt = "signext",
    SafeStack = "safestack",
    SanitizeAddress = "sanitize_address",
    SanitizeHWAddress = "sanitize_hwaddress",
    SanitizeMemTag = "sanitize_memtag",
    SanitizeMemory = "sanitize_memory",
    SanitizeNumericalStability = "sanitize_numerical_stability",
    SanitizeThread = "sanitize_thread",
    ShadowCallStack = "shadowcallstack",
    SkipProfile = "skipprofile",
    Speculatable = "speculatable",
    SpeculativeLoadHardening = "speculative_load_hardening",
    StackProtect = "ssp",
    StackProtectReq = "sspreq",
    StackProtectStrong = "sspstrong",
    StrictFP = "strictfp",
    SwiftAsync = "swiftasync",
    SwiftError = "swifterror",
    SwiftSelf = "swiftself",
    WillReturn = "willreturn",
    Writable = "writable",
    WriteOnly = "writeonly",
    ZExt = "zeroext",
}

/// Access kind for one memory location in [`MemoryEffects`].
#[derive(Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Debug, Default)]
pub enum ModRef {
    /// `none`.
    #[default]
    None,
    /// `read`.
    Read,
    /// `write`.
    Write,
    /// `readwrite`.
    ReadWrite,
}

impl ModRef {
    fn name(self) -> &'static str {
        match self {
            ModRef::None => "none",
            ModRef::Read => "read",
            ModRef::Write => "write",
            ModRef::ReadWrite => "readwrite",
        }
    }
    fn union(self, o: ModRef) -> ModRef {
        match (self as u8) | (o as u8) {
            0 => ModRef::None,
            1 => ModRef::Read,
            2 => ModRef::Write,
            _ => ModRef::ReadWrite,
        }
    }
}

/// The `memory(...)` attribute: access per location.
#[derive(Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Debug, Default)]
pub struct MemoryEffects {
    /// `argmem:` — memory reachable from pointer arguments.
    pub argmem: ModRef,
    /// `inaccessiblemem:` — memory not accessible to the module.
    pub inaccessiblemem: ModRef,
    /// Everything else (the default access kind).
    pub other: ModRef,
}

impl MemoryEffects {
    /// The same access for every location (`memory(none)`, `memory(read)`).
    pub fn all(m: ModRef) -> Self {
        MemoryEffects { argmem: m, inaccessiblemem: m, other: m }
    }

    /// Only argument memory (`memory(argmem: ...)`).
    pub fn argmem_only(m: ModRef) -> Self {
        MemoryEffects { argmem: m, ..Self::default() }
    }

    fn write<W: fmt::Write + ?Sized>(self, w: &mut W) -> fmt::Result {
        w.write_str("memory(")?;
        let other = self.other;
        let union = self.argmem.union(self.inaccessiblemem).union(self.other);
        let mut first = true;
        if other != ModRef::None || union == other {
            first = false;
            w.write_str(other.name())?;
        }
        for (label, mr) in [("argmem: ", self.argmem), ("inaccessiblemem: ", self.inaccessiblemem)]
        {
            if mr == other {
                continue;
            }
            if !first {
                w.write_str(", ")?;
            }
            first = false;
            w.write_str(label)?;
            w.write_str(mr.name())?;
        }
        w.write_char(')')
    }
}

/// `uwtable` kind.
#[derive(Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub enum UwTableKind {
    /// `uwtable(sync)`.
    Sync,
    /// `uwtable` (asynchronous; the default spelling).
    Async,
}

/// One attribute. Variant order is the sort order (see module docs).
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub enum Attr {
    /// An attribute without arguments (`noundef`, `nounwind`, ...).
    Flag(AttrKind),
    /// `byref(T)`.
    ByRef(TypeId),
    /// `byval(T)`.
    ByVal(TypeId),
    /// `elementtype(T)` (inline asm indirect operands).
    ElementType(TypeId),
    /// `inalloca(T)`.
    InAlloca(TypeId),
    /// `preallocated(T)`.
    Preallocated(TypeId),
    /// `sret(T)`.
    SRet(TypeId),
    /// `align N` (parameter/return) or `align N` on functions.
    Align(u64),
    /// `allocsize(N[, M])`.
    AllocSize(u32, Option<u32>),
    /// `dereferenceable(N)`.
    Dereferenceable(u64),
    /// `dereferenceable_or_null(N)`.
    DereferenceableOrNull(u64),
    /// `memory(...)`.
    Memory(MemoryEffects),
    /// `alignstack(N)`.
    AlignStack(u64),
    /// `uwtable` / `uwtable(sync)`.
    UwTable(UwTableKind),
    /// `vscale_range(min, max)`.
    VScaleRange(u32, u32),
    /// A string attribute `"key"` or `"key"="value"` (empty value prints
    /// the key alone).
    Str(Box<str>, Box<str>),
}

impl Attr {
    /// `"key"="value"`.
    pub fn string(key: &str, value: &str) -> Attr {
        Attr::Str(key.into(), value.into())
    }

    /// Print in textual IR syntax.
    pub(crate) fn write<W: fmt::Write + ?Sized>(
        &self,
        w: &mut W,
        types: &TypeTable,
    ) -> fmt::Result {
        let ty_attr = |w: &mut W, name: &str, t: TypeId| -> fmt::Result {
            write!(w, "{name}(")?;
            types.write(w, t)?;
            w.write_char(')')
        };
        match self {
            Attr::Flag(k) => w.write_str(k.name()),
            Attr::ByRef(t) => ty_attr(w, "byref", *t),
            Attr::ByVal(t) => ty_attr(w, "byval", *t),
            Attr::ElementType(t) => ty_attr(w, "elementtype", *t),
            Attr::InAlloca(t) => ty_attr(w, "inalloca", *t),
            Attr::Preallocated(t) => ty_attr(w, "preallocated", *t),
            Attr::SRet(t) => ty_attr(w, "sret", *t),
            Attr::Align(n) => write!(w, "align {n}"),
            Attr::AllocSize(a, None) => write!(w, "allocsize({a})"),
            Attr::AllocSize(a, Some(b)) => write!(w, "allocsize({a},{b})"),
            Attr::Dereferenceable(n) => write!(w, "dereferenceable({n})"),
            Attr::DereferenceableOrNull(n) => write!(w, "dereferenceable_or_null({n})"),
            Attr::Memory(m) => m.write(w),
            Attr::AlignStack(n) => write!(w, "alignstack({n})"),
            Attr::UwTable(UwTableKind::Async) => w.write_str("uwtable"),
            Attr::UwTable(UwTableKind::Sync) => w.write_str("uwtable(sync)"),
            Attr::VScaleRange(a, b) => write!(w, "vscale_range({a},{b})"),
            Attr::Str(k, v) => {
                w.write_char('"')?;
                crate::print::write_escaped(w, k.as_bytes())?;
                w.write_char('"')?;
                if !v.is_empty() {
                    w.write_str("=\"")?;
                    crate::print::write_escaped(w, v.as_bytes())?;
                    w.write_char('"')?;
                }
                Ok(())
            }
        }
    }

    /// Same attribute "slot": two attributes with equal keys cannot both be
    /// in one set (the later one wins).
    fn same_key(&self, o: &Attr) -> bool {
        match (self, o) {
            (Attr::Str(a, _), Attr::Str(b, _)) => a == b,
            (Attr::Flag(a), Attr::Flag(b)) => a == b,
            _ => std::mem::discriminant(self) == std::mem::discriminant(o),
        }
    }
}

impl From<AttrKind> for Attr {
    fn from(k: AttrKind) -> Attr {
        Attr::Flag(k)
    }
}

/// Interned attribute sets. Id 0 is the empty set.
#[derive(Debug)]
pub(crate) struct AttrTable {
    sets: Vec<Box<[Attr]>>,
    map: HashMap<Box<[Attr]>, AttrSetId>,
}

impl AttrTable {
    pub(crate) fn new() -> Self {
        let mut t = AttrTable { sets: Vec::new(), map: HashMap::new() };
        t.sets.push(Box::new([]));
        t.map.insert(Box::new([]), AttrSetId::EMPTY);
        t
    }

    /// Canonicalize (later duplicates win, then sort) and intern.
    pub(crate) fn intern(&mut self, attrs: &[Attr]) -> AttrSetId {
        let mut v: Vec<Attr> = Vec::with_capacity(attrs.len());
        for a in attrs {
            if let Some(slot) = v.iter_mut().find(|x| x.same_key(a)) {
                *slot = a.clone();
            } else {
                v.push(a.clone());
            }
        }
        v.sort();
        let b = v.into_boxed_slice();
        if let Some(&id) = self.map.get(&b) {
            return id;
        }
        let id = AttrSetId(self.sets.len() as u32);
        self.sets.push(b.clone());
        self.map.insert(b, id);
        id
    }

    pub(crate) fn get(&self, id: AttrSetId) -> &[Attr] {
        &self.sets[id.index()]
    }

    pub(crate) fn len(&self) -> usize {
        self.sets.len()
    }
}

/// Attributes of a function or call site: function, return and per
/// parameter sets.
#[derive(Clone, PartialEq, Eq, Hash, Debug, Default)]
pub struct AttrList {
    /// Function attributes (printed as a `#N` group).
    pub func: AttrSetId,
    /// Return attributes (printed before the return type).
    pub ret: AttrSetId,
    /// Parameter attributes by position; missing entries are empty.
    pub params: Vec<AttrSetId>,
}

impl Default for AttrSetId {
    fn default() -> Self {
        AttrSetId::EMPTY
    }
}

impl AttrList {
    /// Attributes of parameter `i` (empty when absent).
    pub fn param(&self, i: usize) -> AttrSetId {
        self.params.get(i).copied().unwrap_or(AttrSetId::EMPTY)
    }

    /// Set the attributes of parameter `i`, growing the list.
    pub fn set_param(&mut self, i: usize, s: AttrSetId) {
        if self.params.len() <= i {
            self.params.resize(i + 1, AttrSetId::EMPTY);
        }
        self.params[i] = s;
    }
}
