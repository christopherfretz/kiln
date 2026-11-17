//! Record kind 2 of the `.mason.meta` unit: one row per symbol the
//! object defines for a definition that may legally appear in several
//! units (mortar `spec/odr.md` O-14, O-15; spec/codegen.md "Link-time
//! metadata (`.mason.meta`)", "Kind 2"; WP-1581, K-3a).
//!
//! The rows are read after module emission from what the module already
//! holds ([`Cg::fns`], [`Cg::sfns`], [`Cg::gvars`] and the IR linkage each
//! symbol was given): nothing is recorded while emitting, and nothing here
//! runs without `-fmason-metadata`. The row set follows the emitted
//! linkage, so it cannot disagree with the symbols: a `linkonce_odr` or
//! `weak_odr` definition has a `VAGUE` row; a strong one has a row only
//! when O-14 names it (an explicit specialization, K13; the out-of-line
//! definition of a `static constexpr` data member, N22).
//!
//! The hash of this package covers the declared type only and every row
//! is `PARTIAL`: the definition's body is K-3b's serializer, wired in by
//! K-3c.

use std::collections::BTreeMap;

use kiln_abi::mangle;
use kiln_abi::odr::{fnv1a_128, fnv1a64};
use kiln_ast::decl::{DeclFlags, DeclKind, FnStorage};
use kiln_ast::expr::ExprKind;
use kiln_ast::templates::SpecKind;
use kiln_ast::{DeclId, QualType};
use kiln_llvm::{Attr, AttrKind, Linkage, Value, ValueRef};
use kiln_source::fx::FxHashMap;

use crate::mason_meta::{Block, StrTab};
use crate::module::Cg;

/// Record kind 2, the definitions (O-15), and its row layout version.
pub const KIND_DEFINITIONS: u16 = 2;
const DEFINITIONS_VERSION: u16 = 1;
/// A kind 2 row: `{u64 key; u8 hash[16]; u32 name; u16 class; u16 flags;
/// u32 module; u32 file; u32 line; u32 reserved}`.
const DEFINITION_ROW_SIZE: u32 = 48;

/// `class`: a function.
pub const CLASS_FUNCTION: u16 = 1;
/// `class`: a variable.
pub const CLASS_VARIABLE: u16 = 2;

/// O-15 flag bit 0: vague linkage (`linkonce_odr` / `weak_odr` in a
/// comdat).
pub const DEF_VAGUE: u16 = 1;
/// Bit 1: an implicit instantiation (or a member of one).
pub const DEF_IMPLICIT_INST: u16 = 2;
/// Bit 2: an explicit instantiation definition (or a member it covers).
pub const DEF_EXPLICIT_INST: u16 = 4;
/// Bit 3: an explicit specialization (or a member of an explicitly
/// specialized class).
pub const DEF_EXPLICIT_SPEC: u16 = 8;
/// Bit 5: the hasher skipped something its scheme covers (every row of
/// this package: the body is not hashed yet).
pub const DEF_PARTIAL: u16 = 32;
/// Bit 8: defined in a system header.
pub const DEF_SYSTEM: u16 = 256;

/// Kind 1's `odr.scheme` (O-11, mortar's ruling 3 of 11-17): the
/// definition serialization of the kind 2 `hash`. `d0`: the declared
/// type only, every row `PARTIAL`. K-3c switches this to `kiln_abi`'s
/// constant of the body serialization.
pub const DEF_SCHEME: &str = "kiln-odr-1.d0";

/// What a type with a per-TU-numbered component hashes as (O-10: one
/// placeholder, a miss).
const PER_TU_PLACEHOLDER: &str = "?per-tu";
/// The token that stands for the body this scheme does not hash.
const BODY_NOT_HASHED: &str = "body not hashed";

/// One kind 2 row before encoding (O-15). `name` and `module` are 0 in
/// this package (anchored rows; no named modules yet), `reserved` 0.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct DefRow {
    /// FNV-1a 64 of the symbol's name (MT-13).
    pub key: u64,
    /// `CLASS_FUNCTION` or `CLASS_VARIABLE`.
    pub class: u16,
    /// `DEF_*` bits.
    pub flags: u16,
    /// The 128-bit definition hash, stored little-endian.
    pub hash: u128,
    /// The definition's file after the prefix maps (empty: unknown).
    pub file: String,
    /// The definition's line (0: unknown).
    pub line: u32,
}

/// The kind 2 block of `rows` (O-15, MT-11): sorted by `key`, then the
/// remaining fields (a total order; exact duplicates dropped), files
/// added to `st` in row order.
#[must_use]
pub fn definitions_block(rows: &[DefRow], st: &mut StrTab) -> Block {
    let mut rows: Vec<&DefRow> = rows.iter().collect();
    rows.sort();
    rows.dedup();
    let mut body = Vec::with_capacity(rows.len() * DEFINITION_ROW_SIZE as usize);
    for r in &rows {
        let file = st.add(r.file.as_bytes());
        body.extend_from_slice(&r.key.to_le_bytes());
        body.extend_from_slice(&r.hash.to_le_bytes());
        body.extend_from_slice(&0u32.to_le_bytes()); // name: anchored
        body.extend_from_slice(&r.class.to_le_bytes());
        body.extend_from_slice(&r.flags.to_le_bytes());
        body.extend_from_slice(&0u32.to_le_bytes()); // module
        body.extend_from_slice(&file.to_le_bytes());
        body.extend_from_slice(&r.line.to_le_bytes());
        body.extend_from_slice(&0u32.to_le_bytes()); // reserved
    }
    Block {
        kind: KIND_DEFINITIONS,
        version: DEFINITIONS_VERSION,
        row_size: DEFINITION_ROW_SIZE,
        row_count: u32::try_from(rows.len()).unwrap_or(u32::MAX),
        body,
    }
}

/// Whether `enc` (a `<type>` encoding or a symbol name) has a component
/// numbered or named per TU: `_GLOBAL__N`, kiln's `$_N`, a local entity
/// `Z...E`, an unnamed `Ut`/`Ul` (conservative: any of them makes the
/// type hash as the placeholder, a miss, never a false finding).
fn has_per_tu_component(enc: &str) -> bool {
    let b = enc.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i].is_ascii_digit() {
            let start = i;
            while i < b.len() && b[i].is_ascii_digit() {
                i += 1;
            }
            let n: usize = enc[start..i].parse().unwrap_or(0);
            let name = &b[i..(i + n).min(b.len())];
            if name.starts_with(b"_GLOBAL__N") || name.starts_with(b"$") {
                return true;
            }
            i += n;
            continue;
        }
        match b[i] {
            b'Z' => return true,
            b'U' if matches!(b.get(i + 1), Some(b't' | b'l')) => return true,
            // `L_Z <encoding> E`: the encoding's own `Z` is not a local
            // name.
            b'L' if b.get(i + 1) == Some(&b'_') && b.get(i + 2) == Some(&b'Z') => i += 2,
            // A literal `L <type> <value> E`: skip the value's digits.
            b'L' if b.get(i + 1).is_some_and(u8::is_ascii_lowercase) => {
                while i < b.len() && b[i] != b'E' {
                    i += 1;
                }
            }
            b'$' => return true,
            _ => {}
        }
        i += 1;
    }
    false
}

/// The hash of this package (scheme `d0`): FNV-1a 128 over the class tag,
/// the declared type's encoding (or the placeholder) and one fixed token
/// for the body.
fn def_hash(class: u16, ty: &str) -> u128 {
    let tag = if class == CLASS_FUNCTION { "function" } else { "variable" };
    let ty = if has_per_tu_component(ty) { PER_TU_PLACEHOLDER } else { ty };
    fnv1a_128(&format!("{tag}\0{ty}\0{BODY_NOT_HASHED}"))
}

/// A candidate symbol: (name as the symbol table spells it, the first
/// declaration of the definition, variable, vague).
type Cand = (String, DeclId, bool, bool);

/// Prefixes of owned and compiler-generated symbols, which get no row
/// (ruling 1: static locals, guards, reference temporaries; for every
/// class TLS wrappers, thunks, vtables, VTTs, typeinfo, template
/// parameter objects, structured binding holders). The first three
/// apply to variables only.
const OWNED: [&str; 14] = [
    "_ZZ", "_ZGV", "_ZGR", "_ZTW", "_ZTH", "_ZTV", "_ZTT", "_ZTI", "_ZTS", "_ZTh", "_ZTv", "_ZTc",
    "_ZTA", "_ZDC",
];

impl Cg<'_> {
    /// The kind 2 rows of this C++ unit (O-14, O-15; spec/codegen.md
    /// "Kind 2"): one per symbol the object defines that names a
    /// definition of O-14's list. Symbols are taken from the declaration
    /// maps with the linkage the emitter gave them; a symbol whose
    /// candidates give different rows gets none.
    pub(crate) fn mason_def_rows(&mut self) -> Vec<DefRow> {
        let mut cands: Vec<Cand> = Vec::new();
        for (&first, &f) in &self.fns {
            if self.m.resolve(Value::func(f)) != Value::func(f) {
                continue;
            }
            let func = self.m.func(f);
            if func.is_declaration() {
                continue;
            }
            if let Some(vague) = row_linkage(func.linkage)
                && !(vague && self.always_inline(f))
            {
                cands.push((func.name().to_owned(), first, false, vague));
            }
        }
        for (&(first, _), &(val, f)) in &self.sfns {
            let (name, linkage) = match self.m.resolve(val).get() {
                ValueRef::Func(g) => {
                    let func = self.m.func(g);
                    // An `always_inline` structor (libstdc++'s allocators).
                    if func.is_declaration()
                        || (row_linkage(func.linkage) == Some(true) && self.always_inline(g))
                    {
                        continue;
                    }
                    (func.name(), func.linkage)
                }
                ValueRef::Alias(a) => {
                    let al = self.m.alias(a);
                    // The variant is an alias of a defined function.
                    let defined = matches!(
                        self.m.resolve(al.aliasee()).get(),
                        ValueRef::Func(g) if !self.m.func(g).is_declaration()
                    );
                    if al.is_ifunc() || !defined || self.m.func(f).is_declaration() {
                        continue;
                    }
                    (al.name(), al.linkage)
                }
                _ => continue,
            };
            // A variant replaced by another class's symbol (a base-object
            // destructor replaced by its base's) is not this definition's.
            if self.reps.get(name).is_some_and(|&r| r != first) {
                continue;
            }
            if let Some(vague) = row_linkage(linkage) {
                cands.push((name.to_owned(), first, false, vague));
            }
        }
        for (&first, &g) in &self.gvars {
            if self.m.resolve(Value::global(g)) != Value::global(g) {
                continue;
            }
            let gv = self.m.global(g);
            if gv.initializer().is_none() {
                continue;
            }
            if let Some(vague) = row_linkage(gv.linkage) {
                cands.push((gv.name().to_owned(), first, true, vague));
            }
        }
        // Symbol order, so that anything the mangler numbers is numbered
        // the same way in every run (MT-38).
        cands.sort();
        let mut types: FxHashMap<QualType, String> = FxHashMap::default();
        let mut by_symbol: BTreeMap<String, Option<DefRow>> = BTreeMap::new();
        for (name, first, var, vague) in cands {
            let name = name.strip_prefix('\u{1}').unwrap_or(&name).to_owned();
            let Some(row) = self.mason_def_row(&name, first, var, vague, &mut types) else {
                continue;
            };
            match by_symbol.get_mut(&name) {
                None => {
                    by_symbol.insert(name, Some(row));
                }
                Some(slot) => {
                    if slot.as_ref() != Some(&row) {
                        *slot = None;
                    }
                }
            }
        }
        by_symbol.into_values().flatten().collect()
    }

    /// Whether function `f` is `alwaysinline`. A vague one gets no row:
    /// the always-inliner deletes it at every `-O` once its calls are
    /// inlined, so the object does not define it (a miss when its address
    /// is taken).
    fn always_inline(&self, f: kiln_llvm::FuncId) -> bool {
        self.m.attrs_of(self.m.func(f).attrs.func).contains(&Attr::Flag(AttrKind::AlwaysInline))
    }

    /// The row of symbol `name` for the definition whose first
    /// declaration is `first`, or `None` when O-14 gives it none.
    fn mason_def_row(
        &mut self,
        name: &str,
        first: DeclId,
        var: bool,
        vague: bool,
        types: &mut FxHashMap<QualType, String>,
    ) -> Option<DefRow> {
        let ast = self.ast;
        // Names numbered per TU: X15 (a lambda in a declarator gives `$_N`).
        if name.is_empty()
            || name.starts_with("llvm.")
            || name.contains("$_")
            || name.contains("_GLOBAL__N")
            || (var && OWNED.iter().any(|p| name.starts_with(p)))
            || (!var && OWNED[3..].iter().any(|p| name.starts_with(p)))
        {
            return None;
        }
        let n = ast.decls.get(first);
        let kind = n.kind();
        let def = if var {
            if !matches!(kind, DeclKind::Var | DeclKind::VarTemplateSpecialization) {
                return None;
            }
            // A static local is its function's (ruling 1).
            let local = n.ctx().is_some_and(|c| {
                ast.decls.get(c).kind().payload() == kiln_ast::decl::PayloadClass::Function
            });
            if local {
                return None;
            }
            ast.definition(first).unwrap_or(first)
        } else {
            if !matches!(
                kind,
                DeclKind::Function
                    | DeclKind::CXXMethod
                    | DeclKind::CXXConversion
                    | DeclKind::CXXConstructor
                    | DeclKind::CXXDestructor
            ) {
                return None;
            }
            let def = ast.definition(first).unwrap_or(first);
            if self.closure_forwarder(def) || self.lambda_in_default_argument(def) {
                return None;
            }
            def
        };
        if ast.weakref_target(first).is_some() {
            return None;
        }
        let mut flags = DEF_PARTIAL | self.def_spec_flags(first);
        if vague {
            flags |= DEF_VAGUE;
        } else {
            // A strong definition has a row only when O-14 names it: an
            // explicit specialization (K13) or the out-of-line definition
            // of a `static constexpr` data member (N22).
            let constexpr_member =
                var && n.ctx().is_some_and(|c| {
                    ast.decls.get(c).kind().payload() == kiln_ast::decl::PayloadClass::Record
                }) && ast
                    .redecls_of(first)
                    .any(|r| ast.decls.var(r).is_some_and(|v| v.spec.is_constexpr()));
            if flags & DEF_EXPLICIT_SPEC == 0 && !constexpr_member {
                return None;
            }
        }
        let ty = ast.decls.get(def).ty()?;
        let enc = if let Some(t) = types.get(&ty) {
            t.clone()
        } else {
            let t = mangle::mangle_declared_type(ast, &mut self.anon, ty);
            types.insert(ty, t.clone());
            t
        };
        let class = if var { CLASS_VARIABLE } else { CLASS_FUNCTION };
        // The place: an instantiation's template definition; an
        // implicitly defined member's class; an explicit specialization's
        // own definition.
        let mut at = def;
        if flags & DEF_EXPLICIT_SPEC != 0 {
            // `def` itself.
        } else if let Some(p) = ast.instantiated_from(first).and_then(|p| ast.definition(p)) {
            at = p;
        } else if ast.decls.get(def).has(DeclFlags::IMPLICIT)
            && let Some(c) = ast.decls.get(def).ctx()
        {
            at = ast.definition(c).unwrap_or(c);
        }
        let (file, line, system) = mason_place(self.sm, self.opts, ast.decls.get(at).loc());
        if system {
            flags |= DEF_SYSTEM;
        }
        Some(DefRow {
            key: fnv1a64(name.as_bytes()),
            class,
            flags,
            hash: def_hash(class, &enc),
            file,
            line,
        })
    }

    /// `IMPLICIT_INST`, `EXPLICIT_INST` or `EXPLICIT_SPEC` of the
    /// definition of `first` ([temp.inst], [temp.explicit],
    /// [temp.expl.spec]): its own specialization kind, else that of the
    /// innermost enclosing class template specialization. A member of an
    /// explicitly specialized class is part of that explicit
    /// specialization's definition (its symbol clashes with the implicit
    /// instantiation of the primary template's member, K13), so it is
    /// `EXPLICIT_SPEC`; an instantiated member template of such a class
    /// is an instantiation.
    fn def_spec_flags(&self, first: DeclId) -> u16 {
        let ast = self.ast;
        let of = |k: SpecKind| match k {
            SpecKind::ExplicitSpecialization => DEF_EXPLICIT_SPEC,
            SpecKind::ExplicitInstantiationDefinition => DEF_EXPLICIT_INST,
            SpecKind::Undeclared
            | SpecKind::ImplicitInstantiation
            | SpecKind::ExplicitInstantiationDeclaration => DEF_IMPLICIT_INST,
        };
        if self.explicit_inst_kind(first) == Some(SpecKind::ExplicitInstantiationDefinition) {
            return DEF_EXPLICIT_INST;
        }
        let own = |d: DeclId| {
            ast.spec_info(d)
                .map(|(_, s)| s.kind)
                .or_else(|| ast.member_spec_info(d).map(|(_, m)| m.kind))
        };
        if let Some(k) = own(first) {
            return of(k);
        }
        let mut ctx = ast.decls.get(first).ctx();
        for _ in 0..32 {
            let Some(c) = ctx else { break };
            if !matches!(
                ast.decls.get(c).kind(),
                DeclKind::CXXRecord | DeclKind::ClassTemplateSpecialization
            ) {
                break;
            }
            if let Some(k) = own(ast.first(c)) {
                return of(k);
            }
            ctx = ast.decls.get(c).ctx();
        }
        0
    }

    /// A closure's static invoker or its conversion function to a
    /// function pointer (O-14: no row; synthesized forwarding).
    fn closure_forwarder(&self, def: DeclId) -> bool {
        let ast = self.ast;
        let n = ast.decls.get(def);
        let Some(rec) = n.ctx() else { return false };
        if ast.closure_lambda(rec).is_none() {
            return false;
        }
        n.kind() == DeclKind::CXXConversion
            || ast.decls.function(def).is_some_and(|f| f.spec.storage() == FnStorage::Static)
            || ast.closure_thunk(def).is_some()
    }

    /// X15: a parameter of the definition (or of the template it was
    /// instantiated from) has a default argument holding a lambda.
    fn lambda_in_default_argument(&self, def: DeclId) -> bool {
        let ast = self.ast;
        let mut cur = Some(def);
        let mut stack = Vec::new();
        for _ in 0..8 {
            let Some(d) = cur else { return false };
            for r in ast.redecls_of(d) {
                let Some(f) = ast.decls.function(r) else { continue };
                for p in ast.lists.iter::<DeclId>(f.params) {
                    if let Some(e) = ast.decls.param(p).and_then(|x| x.default_arg) {
                        stack.push(e);
                    }
                }
            }
            while let Some(e) = stack.pop() {
                if ast.exprs.get(e).kind() == ExprKind::LambdaExpr {
                    return true;
                }
                ast.exprs.children_into(e, &ast.lists, &mut stack);
            }
            cur = ast.instantiated_from(ast.first(d)).or_else(|| {
                ast.spec_info(d).and_then(|(_, s)| ast.template_info(s.template)?.pattern)
            });
        }
        false
    }
}

/// The file and line of location `loc` (presumed, after the prefix maps
/// of `opts`) and whether it is in a system header: a kind 2 or kind 3
/// row's `file`, `line` and `SYSTEM`.
pub(crate) fn mason_place(
    sm: Option<&kiln_source::SourceManager>,
    opts: &crate::CodegenOptions,
    loc: kiln_source::Loc,
) -> (String, u32, bool) {
    let (mut file, mut line, mut system) = (String::new(), 0, false);
    if let Some(sm) = sm {
        system = sm.is_in_system_header(loc);
        if let Some(p) = sm.presumed_line_col(loc) {
            file = opts.debug.remap(&sm.presumed_file_name(&p, loc));
            line = p.line;
        }
    }
    (file, line, system)
}

/// Whether a defined symbol of linkage `l` may have a kind 2 row, and if
/// so whether it is vague (`Some(true)`: `linkonce_odr`, `weak_odr`) or
/// strong (`Some(false)`: `external`). `None`: plain `weak` (N5),
/// `linkonce`, `common`, `available_externally` (not defined here) and
/// every linkage without an external name.
fn row_linkage(l: Linkage) -> Option<bool> {
    match l {
        Linkage::LinkOnceOdr | Linkage::WeakOdr => Some(true),
        Linkage::External => Some(false),
        Linkage::LinkOnce
        | Linkage::Weak
        | Linkage::Common
        | Linkage::ExternWeak
        | Linkage::AvailableExternally
        | Linkage::Internal
        | Linkage::Private
        | Linkage::Appending => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn per_tu_components() {
        assert!(!has_per_tu_component("FivE"));
        assert!(!has_per_tu_component("FvRKN2ns3BoxIiEEE"));
        assert!(has_per_tu_component("FvN12_GLOBAL__N_11AE"));
        assert!(has_per_tu_component("FvN3$_0E"));
        assert!(has_per_tu_component("FvZ1fvE1SE"));
        assert!(has_per_tu_component("FvN1AUt_E"));
        // A source name that only contains the letters is not one.
        assert!(!has_per_tu_component("Fv3UtZE"));
        assert!(!has_per_tu_component("FvN1AILi5EEE"));
    }

    #[test]
    fn hash_is_the_type_and_the_class() {
        assert_eq!(def_hash(CLASS_FUNCTION, "FivE"), def_hash(CLASS_FUNCTION, "FivE"));
        assert_ne!(def_hash(CLASS_FUNCTION, "FivE"), def_hash(CLASS_FUNCTION, "FlvE"));
        assert_ne!(def_hash(CLASS_FUNCTION, "i"), def_hash(CLASS_VARIABLE, "i"));
        // Every per-TU type hashes as the one placeholder.
        assert_eq!(def_hash(CLASS_FUNCTION, "FvN3$_0E"), def_hash(CLASS_FUNCTION, "FvN3$_1E"));
        // The scheme's own golden: K-3c changes it with the scheme name.
        assert_eq!(DEF_SCHEME, "kiln-odr-1.d0");
        assert_eq!(def_hash(CLASS_FUNCTION, "FivE"), fnv1a_128("function\0FivE\0body not hashed"));
    }

    #[test]
    fn row_layout() {
        let r = DefRow {
            key: 0x0102_0304_0506_0708,
            class: CLASS_VARIABLE,
            flags: DEF_VAGUE | DEF_PARTIAL,
            hash: 0x1112_1314_1516_1718_191a_1b1c_1d1e_1f20,
            file: "a.h".to_owned(),
            line: 7,
        };
        let mut st = StrTab::default();
        let b = definitions_block(&[r.clone(), r.clone()], &mut st);
        assert_eq!((b.kind, b.version, b.row_size, b.row_count), (2, 1, 48, 1));
        let x = &b.body;
        assert_eq!(x.len(), 48);
        assert_eq!(u64::from_le_bytes(x[0..8].try_into().unwrap()), r.key);
        assert_eq!(u128::from_le_bytes(x[8..24].try_into().unwrap()), r.hash);
        assert_eq!(&x[24..28], &[0; 4], "name: anchored");
        assert_eq!(u16::from_le_bytes([x[28], x[29]]), CLASS_VARIABLE);
        assert_eq!(u16::from_le_bytes([x[30], x[31]]), DEF_VAGUE | DEF_PARTIAL);
        assert_eq!(&x[32..36], &[0; 4], "module");
        assert_eq!(u32::from_le_bytes(x[36..40].try_into().unwrap()), 1, "the file string");
        assert_eq!(u32::from_le_bytes(x[40..44].try_into().unwrap()), 7);
        assert_eq!(&x[44..48], &[0; 4], "reserved");
    }
}
