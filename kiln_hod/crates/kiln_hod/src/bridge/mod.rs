//! The bridge (hod's design 0004, seam K3a): `kiln_llvm::Module` copied
//! into `hod_ir` in process, without printing the module and reading the
//! text back.
//!
//! The bridge is specified by equivalence with hod's text reader
//! (`hod_ll`): for every module M it takes, the `hod_ir` module and bodies
//! it builds print (`hod_ir`'s printer) exactly as those `hod_ll` builds from
//! `M.to_text()`, and record the same unsupported constructs, in the same
//! order, against the same symbols. Each mapping below therefore mirrors
//! the reader's, construct for construct: the order symbols are created
//! in (variables, functions, aliases, ifuncs), the order attribute sets
//! are interned in (kiln's `#N` numbering), the notes a construct records
//! and the order of its operands (a value a body uses before defining it
//! is reserved where the text names it first). What is not transmitted:
//! names of local values and blocks and the `source_filename`, which the
//! reader drops as well; the one difference the design names (an
//! unlabelled `blockaddress`, which the text cannot name) is checked by
//! [`eq`].
//!
//! What the bridge does not map makes it give up on the whole module
//! ([`Fallback`], with a stable reason): the caller then takes the text
//! path for that module. A module is never half bridged.
#![allow(clippy::too_many_lines, reason = "one mapping per construct, as the reader's")]

mod body;
mod debug;
pub mod eq;

use std::borrow::Cow;

use hod_ir::kit::{
    self, AttrUse, BranchUpgrade, GlobalAttachment, NamedMetadataKind, SpecialDeliveryError,
    SpecialGlobal, TypedAttrError, ValueKeyword,
};
use hod_ir::{
    AttrSyntax, CallConv, ComdatId, ComdatKind, Const, ConstId, ConstOp, FlagValue, FnAttrs,
    FnAttrsId, FunctionDesc, IFuncDesc, Linkage, ParamAttrs, SymId, TargetSpec, TlsModel, TypeId,
    UnnamedAddr, UwTable, VariableDesc, Visibility,
};
use kiln_llvm as k;

pub use body::Built;

/// Why a module takes the text path: a stable name, such as
/// `opaque-struct` or `const:splat`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fallback(pub Cow<'static, str>);

impl Fallback {
    fn new(why: impl Into<Cow<'static, str>>) -> Fallback {
        Fallback(why.into())
    }
}

impl std::fmt::Display for Fallback {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

type R<T> = Result<T, Fallback>;

/// An error of `hod_ir` where the reader would have stopped with a
/// malformed-input error: the text path reports it.
fn ir<T>(result: hod_ir::IrResult<T>) -> R<T> {
    result.map_err(|e| Fallback(format!("ir:{:?}", e.kind).into()))
}

/// An attribute group as the reader keeps it (`hod_ll::AttrGroup`).
#[derive(Clone, Debug)]
struct Group {
    attrs: FnAttrsId,
    align: Option<u64>,
    rejected: Vec<Box<str>>,
    noinline: bool,
}

/// The module level, built; the bodies follow ([`Bridge::body`]).
pub struct Bridge<'k> {
    k: &'k k::Module,
    /// The module being built: handed to a `hod::Session` after the
    /// module level, or kept for printing ([`eq`]).
    pub module: hod_ir::Module,
    types: Vec<Option<TypeId>>,
    consts: Vec<Option<ConstId>>,
    funcs: Vec<Option<SymId>>,
    globals: Vec<Option<SymId>>,
    aliases: Vec<Option<SymId>>,
    comdats: Vec<Option<ComdatId>>,
    groups: Vec<Option<Group>>,
    pending: Vec<Cow<'static, str>>,
    upgrade: Option<BranchUpgrade>,
    upgraded: Vec<(FnAttrsId, FnAttrsId)>,
    /// The functions with a body, in module order, and their symbols.
    defined: Vec<(k::FuncId, SymId)>,
    /// Debug information (seam K3b).
    dbg: debug::DebugState<'k>,
}

/// Whether `m` carries debug information: `!llvm.dbg.cu`, any `!dbg`
/// attachment, or a debug record. Such a module keeps the text path under
/// `KILN_HOD_BRIDGE_DEBUG=0`.
pub fn has_debug_info(m: &k::Module) -> bool {
    let dbg = |kind: k::MdKindId| m.md_kind_name(kind) == "dbg";
    if m.named_metadata().any(|(name, _)| name == "llvm.dbg.cu") {
        return true;
    }
    if m.global_ids().any(|g| m.global(g).metadata().iter().any(|&(kind, _)| dbg(kind))) {
        return true;
    }
    m.functions().any(|(_, f)| {
        f.metadata().iter().any(|&(kind, _)| dbg(kind))
            || f.layout().iter().flat_map(|&bb| f.block_insts(bb)).any(|&i| {
                f.inst_metadata(i).iter().any(|&(_, kind, _)| dbg(kind))
                    || f.debug_records(i).next().is_some()
            })
    })
}

impl std::fmt::Debug for Bridge<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Bridge").field("defined", &self.defined.len()).finish_non_exhaustive()
    }
}

impl<'k> Bridge<'k> {
    /// The module level of `m`: target, types, symbols, comdats,
    /// attribute groups, named metadata and every global's description,
    /// as `hod_ll::parse_module` builds them from `m.to_text()`.
    /// `default_target` is the reader's (`-mtriple`), for a module that
    /// names none.
    ///
    /// # Errors
    /// What the bridge does not map, or where the reader would fail.
    pub fn new(m: &'k k::Module, default_target: Option<&TargetSpec>) -> R<Bridge<'k>> {
        let target = if m.target_triple.is_empty() {
            default_target.cloned().ok_or_else(|| Fallback::new("no-target"))?
        } else {
            TargetSpec::from_triple(&m.target_triple)
                .map_err(|_| Fallback::new("unsupported-target"))?
        };
        if !m.data_layout.is_empty() && target.check_data_layout(&m.data_layout).is_err() {
            return Err(Fallback::new("data-layout"));
        }
        let mut b = Bridge {
            k: m,
            module: hod_ir::Module::new(target),
            types: vec![None; m.types.len()],
            consts: vec![None; m.num_consts()],
            funcs: vec![None; m.num_functions()],
            globals: vec![None; m.num_globals()],
            aliases: vec![None; m.num_aliases()],
            comdats: vec![None; m.num_comdats()],
            groups: vec![None; m.num_attr_sets()],
            pending: Vec::new(),
            upgrade: None,
            upgraded: Vec::new(),
            defined: Vec::new(),
            dbg: debug::DebugState::default(),
        };
        // The printer writes a chunk's lines as one `module asm` line
        // each, and the reader appends each line.
        for chunk in &m.module_asm {
            for line in chunk.split('\n') {
                b.module.append_module_asm(line.as_bytes());
            }
        }
        b.check_types()?;
        b.number_globals()?;
        b.define_comdats()?;
        b.define_groups()?;
        b.named_metadata()?;
        b.build_debug()?;
        b.flush(None);
        b.items()?;
        let listed = b.dbg.units.is_some();
        ir(kit::finish_debug(&mut b.module, listed))?;
        ir(b.module.verify_symbols())?;
        Ok(b)
    }

    /// The functions with a body, in module order.
    pub fn defined(&self) -> &[(k::FuncId, SymId)] {
        &self.defined
    }

    fn note(&mut self, construct: impl Into<Cow<'static, str>>) {
        let construct = construct.into();
        if !self.pending.contains(&construct) {
            self.pending.push(construct);
        }
    }

    fn flush(&mut self, symbol: Option<SymId>) {
        for construct in self.pending.drain(..) {
            self.module.record_unsupported(&construct, symbol);
        }
    }

    // ---- types ----

    /// An opaque struct is numbered by its place among the printed type
    /// definitions, which the module does not expose: such a module takes
    /// the text path.
    fn check_types(&self) -> R<()> {
        let t = &self.k.types;
        if t.named_structs().iter().any(|&s| t.is_opaque_struct(s)) {
            return Err(Fallback::new("opaque-struct"));
        }
        Ok(())
    }

    fn ty(&mut self, ty: k::TypeId) -> R<TypeId> {
        if let Some(done) = self.types[ty.index()] {
            return Ok(done);
        }
        let types = &self.k.types;
        let mapped = match types.data(ty) {
            k::TypeData::Void => TypeId::VOID,
            k::TypeData::Label => TypeId::LABEL,
            k::TypeData::Metadata => TypeId::METADATA,
            k::TypeData::Token => TypeId::TOKEN,
            k::TypeData::Half => TypeId::HALF,
            k::TypeData::BFloat => TypeId::BFLOAT,
            k::TypeData::Float => TypeId::FLOAT,
            k::TypeData::Double => TypeId::DOUBLE,
            k::TypeData::Fp128 => TypeId::FP128,
            k::TypeData::X86Fp80 => TypeId::X86_FP80,
            &k::TypeData::Int(bits) => match bits {
                1 => TypeId::I1,
                8 => TypeId::I8,
                16 => TypeId::I16,
                32 => TypeId::I32,
                64 => TypeId::I64,
                _ => ir(self.module.types.int(bits))?,
            },
            &k::TypeData::Ptr(0) => TypeId::PTR,
            &k::TypeData::Ptr(space) => ir(self.module.types.ptr(space))?,
            &k::TypeData::Vector { len, elem, scalable } => {
                let elem = self.ty(elem)?;
                if scalable {
                    ir(self.module.types.scalable_vector(elem, len))?
                } else {
                    ir(self.module.types.vector(elem, len))?
                }
            }
            &k::TypeData::Array { len, elem } => {
                let elem = self.ty(elem)?;
                ir(self.module.types.array(elem, len))?
            }
            k::TypeData::Struct { fields, packed } => {
                let packed = *packed;
                let fields = self.tys(fields)?;
                ir(self.module.types.struct_type(&fields, packed))?
            }
            k::TypeData::Named(_) => {
                let Some(fields) = types.struct_fields(ty) else {
                    return Err(Fallback::new("opaque-struct"));
                };
                let packed = types.struct_is_packed(ty);
                let fields = self.tys(fields)?;
                ir(self.module.types.struct_type(&fields, packed))?
            }
            k::TypeData::TargetExt(name) => ir(self.module.types.target_ext(name))?,
            // hod has no `x86_amx` type (AMX tile builtins, WP-1739): the llc route.
            k::TypeData::X86Amx => return Err(Fallback::new("x86-amx")),
            k::TypeData::Function { ret, params, varargs } => {
                let (ret, varargs) = (*ret, *varargs);
                let params = self.tys(params)?;
                let ret = self.ty(ret)?;
                ir(self.module.types.function(ret, &params, varargs))?
            }
        };
        self.types[ty.index()] = Some(mapped);
        Ok(mapped)
    }

    fn tys(&mut self, tys: &[k::TypeId]) -> R<Vec<TypeId>> {
        tys.iter().map(|&t| self.ty(t)).collect()
    }

    // ---- symbols ----

    /// The kiln global value `v` names, as a symbol.
    fn sym_of(&self, v: k::Value) -> Option<SymId> {
        match self.k.resolve(v).get() {
            k::ValueRef::Func(f) => self.funcs[f.index()],
            k::ValueRef::Global(g) => self.globals[g.index()],
            k::ValueRef::Alias(a) => self.aliases[a.index()],
            _ => None,
        }
    }

    /// `number_globals`: every symbol, kind by kind in the reader's
    /// order (variables, functions, aliases, ifuncs), each kind in file
    /// order; `__unnamed_N` for an unnamed external symbol, numbered in
    /// file order (variables, aliases, ifuncs, functions).
    fn number_globals(&mut self) -> R<()> {
        let m = self.k;
        let local = |l: k::Linkage| matches!(l, k::Linkage::Internal | k::Linkage::Private);
        // The file-order ordinal of each unnamed symbol.
        let mut ordinal = 0u32;
        let mut next = |name: &str| {
            if name.is_empty() {
                ordinal += 1;
                ordinal
            } else {
                0
            }
        };
        let globals: Vec<(k::GlobalId, u32)> = m
            .global_ids()
            .filter(|&g| SpecialGlobal::from_name(m.global(g).name().as_bytes()).is_none())
            .map(|g| (g, next(m.global(g).name())))
            .collect();
        let aliases: Vec<(k::AliasId, u32)> = m
            .alias_ids()
            .filter(|&a| !m.alias(a).is_ifunc())
            .chain(m.alias_ids().filter(|&a| m.alias(a).is_ifunc()))
            .map(|a| (a, next(m.alias(a).name())))
            .collect();
        let funcs: Vec<(k::FuncId, u32)> =
            m.functions().map(|(f, func)| (f, next(func.name()))).collect();
        for &(g, unnamed) in &globals {
            let v = m.global(g);
            self.globals[g.index()] = Some(self.symbol(v.name(), unnamed, local(v.linkage))?);
        }
        for &(f, unnamed) in &funcs {
            let func = m.func(f);
            let local = local(func.linkage) && !func.is_declaration();
            self.funcs[f.index()] = Some(self.symbol(func.name(), unnamed, local)?);
        }
        for ifunc in [false, true] {
            for &(a, unnamed) in &aliases {
                let alias = m.alias(a);
                if alias.is_ifunc() == ifunc {
                    let sym = self.symbol(alias.name(), unnamed, local(alias.linkage))?;
                    self.aliases[a.index()] = Some(sym);
                }
            }
        }
        Ok(())
    }

    fn symbol(&mut self, name: &str, unnamed: u32, local: bool) -> R<SymId> {
        if !name.is_empty() {
            return ir(self.module.symbol_named(name.as_bytes()));
        }
        if local {
            return ir(self.module.anonymous_symbol());
        }
        let sym = ir(self.module.symbol_named(format!("__unnamed_{unnamed}").as_bytes()))?;
        self.note("symbol:anonymous-external");
        self.flush(Some(sym));
        Ok(sym)
    }

    /// `define_comdats`: the comdats the module prints, in its order (those
    /// of functions, then those of variables, each at its first use).
    fn define_comdats(&mut self) -> R<()> {
        let m = self.k;
        let used: Vec<k::ComdatId> = m
            .functions()
            .filter_map(|(_, f)| f.comdat)
            .chain(m.global_ids().filter_map(|g| m.global(g).comdat))
            .collect();
        for c in used {
            if self.comdats[c.index()].is_some() {
                continue;
            }
            let kind = match m.comdat_kind(c) {
                k::ComdatKind::Any => ComdatKind::Any,
                k::ComdatKind::ExactMatch => ComdatKind::ExactMatch,
                k::ComdatKind::Largest => ComdatKind::Largest,
                k::ComdatKind::NoDeduplicate => ComdatKind::NoDeduplicate,
                k::ComdatKind::SameSize => ComdatKind::SameSize,
            };
            let name = m.comdat_name(c).as_bytes();
            if self.module.find_comdat(name).is_some() {
                return Err(Fallback::new("comdat-redefined"));
            }
            let id = ir(self.module.comdat_named(name))?;
            self.module.set_comdat_kind(id, kind);
            self.comdats[c.index()] = Some(id);
        }
        Ok(())
    }

    // ---- attributes ----

    /// `define_groups`: each attribute set the module prints as a `#N`
    /// group, in `N` order (those of functions in module order, then
    /// those of call sites).
    fn define_groups(&mut self) -> R<()> {
        let m = self.k;
        let mut order: Vec<k::AttrSetId> = Vec::new();
        let mut seen = vec![false; self.groups.len()];
        let mut add = |s: k::AttrSetId| {
            if s != k::AttrSetId::EMPTY && !std::mem::replace(&mut seen[s.index()], true) {
                order.push(s);
            }
        };
        for (_, f) in m.functions() {
            add(f.attrs.func);
        }
        for (_, f) in m.functions() {
            for &bb in f.layout() {
                for &i in f.block_insts(bb) {
                    if let k::InstKind::Call(c) | k::InstKind::Invoke(c) | k::InstKind::CallBr(c) =
                        *f.inst(i).kind()
                    {
                        add(f.call_data(c).attrs().func);
                    }
                }
            }
        }
        for s in order {
            let mut fa = FnAttrs::new();
            let mut align = None;
            self.pending.clear();
            let mut noinline = false;
            let attrs = m.attrs_of(s);
            if attrs.is_empty() {
                return Err(Fallback::new("empty-attribute-group"));
            }
            for a in attrs {
                noinline |= matches!(a, k::Attr::Flag(f) if f.name() == "noinline");
                self.fn_attr(&mut fa, &mut align, a)?;
            }
            let id = ir(self.module.intern_fn_attrs(&fa))?;
            let rejected = self.pending.drain(..).map(|name| name[5..].into()).collect();
            self.groups[s.index()] = Some(Group { attrs: id, align, rejected, noinline });
        }
        Ok(())
    }

    /// `fn_attr`: one function attribute of a group into `fa`.
    fn fn_attr(&mut self, fa: &mut FnAttrs, align: &mut Option<u64>, a: &k::Attr) -> R<()> {
        if let k::Attr::Str(key, value) = a {
            if ir(kit::string_attr(Some(fa), key.as_bytes(), value.as_bytes()))?
                == AttrUse::Rejected
            {
                self.note(kit::attr_construct(key.as_bytes()));
            }
            return Ok(());
        }
        let name = keyword(a);
        let Some(row) = hod_ir::attr_info(AttrSyntax::Keyword, name.as_bytes()) else {
            return Err(Fallback(format!("attr-unknown:{name}").into()));
        };
        match a {
            k::Attr::UwTable(kind) => fa.set_uwtable(match kind {
                k::UwTableKind::Sync => UwTable::Sync,
                k::UwTableKind::Async => UwTable::Async,
            }),
            &k::Attr::AlignStack(bytes) => ir(fa.set_stack_align(bytes))?,
            &k::Attr::Align(bytes) => *align = Some(bytes),
            _ => match ir(kit::fn_keyword(fa, name.as_bytes(), row.class))? {
                AttrUse::Kept | AttrUse::Dropped => {}
                AttrUse::Misplaced => {
                    return Err(Fallback(format!("attr-misplaced:{name}").into()));
                }
                AttrUse::Rejected => self.note(kit::attr_construct(name.as_bytes())),
            },
        }
        Ok(())
    }

    /// `param_attrs`: the attributes of a value (a parameter, a return,
    /// an argument).
    fn value_attrs(&mut self, set: k::AttrSetId) -> R<ParamAttrs> {
        let mut attrs = ParamAttrs::NONE;
        if set == k::AttrSetId::EMPTY {
            return Ok(attrs);
        }
        for a in self.k.attrs_of(set) {
            if let k::Attr::Str(key, value) = a {
                if ir(kit::string_attr(None, key.as_bytes(), value.as_bytes()))?
                    == AttrUse::Rejected
                {
                    self.note(kit::attr_construct(key.as_bytes()));
                }
                continue;
            }
            let name = keyword(a);
            let Some(row) = hod_ir::attr_info(AttrSyntax::Keyword, name.as_bytes()) else {
                return Err(Fallback(format!("attr-unknown:{name}").into()));
            };
            match (kit::value_keyword(name.as_bytes()), a) {
                (ValueKeyword::Flag(flag), _) => attrs = ir(attrs.with(flag))?,
                (
                    ValueKeyword::Typed(mem),
                    &(k::Attr::ByRef(ty)
                    | k::Attr::ByVal(ty)
                    | k::Attr::ElementType(ty)
                    | k::Attr::InAlloca(ty)
                    | k::Attr::Preallocated(ty)
                    | k::Attr::SRet(ty)),
                ) => {
                    let ty = self.ty(ty)?;
                    match kit::add_typed(attrs, mem, ty) {
                        Ok((added, construct)) => {
                            if let Some(construct) = construct {
                                self.note(construct);
                            }
                            attrs = added;
                        }
                        Err(TypedAttrError::TwoTyped) => {
                            return Err(Fallback::new("attr:two-typed"));
                        }
                        Err(TypedAttrError::Ir(e)) => return ir(Err(e)),
                    }
                }
                (ValueKeyword::Align, &k::Attr::Align(bytes)) => {
                    attrs = ir(attrs.with_align(bytes))?;
                }
                (ValueKeyword::StackAlign, &k::Attr::AlignStack(bytes)) => {
                    attrs = ir(attrs.with_stack_align(bytes))?;
                }
                (ValueKeyword::Other, _) => match AttrUse::of_class(row.class) {
                    AttrUse::Misplaced => {
                        return Err(Fallback(format!("attr-misplaced:{name}").into()));
                    }
                    AttrUse::Rejected => self.note(kit::attr_construct(name.as_bytes())),
                    AttrUse::Dropped | AttrUse::Kept => {}
                },
                _ => return Err(Fallback(format!("attr-shape:{name}").into())),
            }
        }
        Ok(attrs)
    }

    /// `function_attrs`: a function's attribute set, from its group.
    fn function_attrs(&mut self, set: k::AttrSetId, defined: bool) -> R<(FnAttrsId, Option<u64>)> {
        let mut align = None;
        let mut id = None;
        if set != k::AttrSetId::EMPTY {
            let group = self.groups[set.index()].clone().ok_or_else(|| Fallback::new("group"))?;
            align = group.align;
            for name in &group.rejected {
                let construct = format!("attr:{name}");
                if !self.pending.iter().any(|noted| **noted == construct) {
                    self.pending.push(construct.into());
                }
            }
            id = Some(group.attrs);
        }
        let mut id = match id {
            Some(id) => id,
            None => ir(self.module.intern_fn_attrs(&FnAttrs::new()))?,
        };
        if defined && let Some(upgrade) = self.upgrade {
            id = ir(upgrade.apply(&mut self.module, id, &mut self.upgraded))?;
        }
        Ok((id, align))
    }

    // ---- named metadata ----

    fn named_metadata(&mut self) -> R<()> {
        let m = self.k;
        let mut upgrade = Some(BranchUpgrade::default());
        for (name, ops) in m.named_metadata() {
            match NamedMetadataKind::of(name.as_bytes()) {
                NamedMetadataKind::ModuleFlags => {
                    for &op in ops {
                        self.module_flag(op, &mut upgrade)?;
                    }
                }
                NamedMetadataKind::LinkerOptions => {
                    for &op in ops {
                        let strings = self.string_tuple(op)?;
                        ir(self.module.add_linker_option(&strings))?;
                    }
                }
                NamedMetadataKind::DependentLibraries => {
                    for &op in ops {
                        let strings = self.string_tuple(op)?;
                        let [library] = strings.as_slice() else {
                            return Err(Fallback::new("dependent-libraries"));
                        };
                        ir(self.module.add_dependent_library(library))?;
                    }
                }
                NamedMetadataKind::Dropped => {}
                NamedMetadataKind::DebugCompileUnits => self.dbg.units = Some(ops),
                NamedMetadataKind::Unsupported => {
                    let construct = kit::named_metadata_construct(name.as_bytes());
                    self.module.record_unsupported(&construct, None);
                }
            }
        }
        if self.module.target().arch() == hod_ir::Arch::Aarch64 {
            self.upgrade = upgrade;
        }
        Ok(())
    }

    fn string_tuple(&self, id: k::MetadataId) -> R<Vec<&'k [u8]>> {
        let m = self.k;
        let k::MdNode::Tuple { ops, .. } = m.md_node(id) else {
            return Err(Fallback::new("metadata-shape"));
        };
        ops.iter()
            .map(|op| match op.map(|op| m.md_node(op)) {
                Some(k::MdNode::String(s)) => Ok(&**s),
                _ => Err(Fallback::new("metadata-shape")),
            })
            .collect()
    }

    fn module_flag(&mut self, id: k::MetadataId, upgrade: &mut Option<BranchUpgrade>) -> R<()> {
        let m = self.k;
        let shape = || Fallback::new("module-flag-shape");
        let k::MdNode::Tuple { ops, .. } = m.md_node(id) else { return Err(shape()) };
        let [Some(behavior), Some(name), value] = **ops else { return Err(shape()) };
        let k::MdNode::Value(_, behavior) = *m.md_node(behavior) else { return Err(shape()) };
        let behavior = self.constant(behavior)?;
        let behavior = self.module.consts.int_value(behavior).ok_or_else(shape)?;
        let k::MdNode::String(name) = m.md_node(name) else { return Err(shape()) };
        let (mut int, mut text) = (None, None);
        match value.map(|v| m.md_node(v)) {
            Some(k::MdNode::String(s)) => text = Some(&**s),
            Some(&k::MdNode::Value(_, v)) => {
                let c = self.constant(v)?;
                int = self.module.consts.int_value(c);
            }
            _ => {}
        }
        ir(kit::module_flag(&mut self.module, behavior, name, int, text, upgrade))
    }

    // ---- constants ----

    /// The constant `v` stands for, of its own type (`parse_typed_const`).
    fn constant(&mut self, v: k::Value) -> R<ConstId> {
        let v = self.k.resolve(v);
        match v.get() {
            k::ValueRef::Const(c) => {
                if let Some(done) = self.consts[c.index()] {
                    return Ok(done);
                }
                let id = self.const_data(c)?;
                self.consts[c.index()] = Some(id);
                Ok(id)
            }
            k::ValueRef::Func(_) | k::ValueRef::Global(_) | k::ValueRef::Alias(_) => {
                let sym = self.sym_of(v).ok_or_else(|| Fallback::new("symbol"))?;
                let ty = self.ty(self.k.const_type(v))?;
                self.address_of(sym, ty)
            }
            _ => Err(Fallback::new("const-operand")),
        }
    }

    fn address_of(&mut self, sym: SymId, ty: TypeId) -> R<ConstId> {
        let address = ir(self.module.consts.addr(sym, 0))?;
        if ty == TypeId::PTR {
            return Ok(address);
        }
        let types = &self.module.types;
        ir(self.module.consts.fold_cast(types, ConstOp::AddrSpaceCast, address, ty))
    }

    fn const_data(&mut self, c: k::ConstId) -> R<ConstId> {
        let m = self.k;
        let data = m.const_data(c);
        let id = match data {
            k::ConstData::Int { ty, value } => {
                let ty = self.ty(*ty)?;
                let (consts, types) = (&mut self.module.consts, &self.module.types);
                match value {
                    &k::IntValue::Small(v) if v >> 64 == 0 => consts.int(types, ty, v as u64),
                    &k::IntValue::Small(v) => {
                        consts.int_from_words(types, ty, &[v as u64, (v >> 64) as u64])
                    }
                    k::IntValue::Big(words) => consts.int_from_words(types, ty, words),
                }
            }
            &k::ConstData::Float { ty, bits } => {
                let ty = self.ty(ty)?;
                self.module.consts.float(&self.module.types, ty, bits)
            }
            &k::ConstData::Null(ty) => {
                let ty = self.ty(ty)?;
                self.module.consts.null(&self.module.types, ty)
            }
            &k::ConstData::Zero(ty) => {
                let ty = self.ty(ty)?;
                self.module.consts.zero(&self.module.types, ty)
            }
            &k::ConstData::Undef(ty) => {
                let ty = self.ty(ty)?;
                self.module.consts.undef(&self.module.types, ty)
            }
            &k::ConstData::Poison(ty) => {
                let ty = self.ty(ty)?;
                self.module.consts.poison(&self.module.types, ty)
            }
            k::ConstData::NoneToken => self.module.consts.none_token(),
            k::ConstData::Array { ty, elems }
            | k::ConstData::Struct { ty, elems }
            | k::ConstData::Vector { ty, elems } => {
                let ty = self.ty(*ty)?;
                let elems = elems.iter().map(|&e| self.constant(e)).collect::<R<Vec<_>>>()?;
                self.module.consts.aggregate(&self.module.types, ty, &elems)
            }
            k::ConstData::Bytes { bytes, .. } => {
                self.module.consts.bytes(&mut self.module.types, bytes)
            }
            k::ConstData::Splat { .. } => return Err(Fallback::new("const:splat")),
            k::ConstData::Gep { source, ops, .. } => {
                let source = self.ty(*source)?;
                let ops = ops.iter().map(|&o| self.constant(o)).collect::<R<Vec<_>>>()?;
                let (base, indices) = ops.split_first().ok_or_else(|| Fallback::new("gep"))?;
                let module = &mut self.module;
                module.consts.fold_gep(&mut module.types, source, *base, indices)
            }
            &k::ConstData::Cast { op, val, ty } => {
                let op = match op {
                    k::CastOp::Trunc => ConstOp::Trunc,
                    k::CastOp::PtrToInt => ConstOp::PtrToInt,
                    k::CastOp::IntToPtr => ConstOp::IntToPtr,
                    k::CastOp::BitCast => ConstOp::BitCast,
                    k::CastOp::AddrSpaceCast => ConstOp::AddrSpaceCast,
                    other => return Err(Fallback(format!("const-expr:{}", other.name()).into())),
                };
                let val = self.constant(val)?;
                let to = self.ty(ty)?;
                self.module.consts.fold_cast(&self.module.types, op, val, to)
            }
            &k::ConstData::Binary { op, lhs, rhs, .. } => {
                let op = match op {
                    k::BinOp::Add => ConstOp::Add,
                    k::BinOp::Sub => ConstOp::Sub,
                    k::BinOp::Mul => ConstOp::Mul,
                    k::BinOp::Xor => ConstOp::Xor,
                    other => return Err(Fallback(format!("const-expr:{}", other.name()).into())),
                };
                let lhs = self.constant(lhs)?;
                let rhs = self.constant(rhs)?;
                self.module.consts.fold_binary(&self.module.types, op, lhs, rhs)
            }
            &k::ConstData::BlockAddress { func, block, .. } => {
                let sym = self.funcs[func.index()].ok_or_else(|| Fallback::new("blockaddress"))?;
                let f = m.func(func);
                let ordinal = f
                    .layout()
                    .iter()
                    .position(|&bb| bb == block)
                    .ok_or_else(|| Fallback::new("blockaddress"))?;
                self.module.consts.block_addr(sym, hod_ir::BlockId::new(ordinal as u32))
            }
            &k::ConstData::DsoLocalEquivalent(v) => {
                let sym = self.sym_of(v).ok_or_else(|| Fallback::new("dso_local_equivalent"))?;
                self.module.consts.dso_local_equivalent(sym)
            }
            &k::ConstData::NoCfi(v) => {
                let sym = self.sym_of(v).ok_or_else(|| Fallback::new("no_cfi"))?;
                self.module.consts.no_cfi(sym)
            }
        };
        ir(id)
    }

    // ---- the items ----

    /// `items`: every global's description, in file order (variables,
    /// aliases, ifuncs, functions).
    fn items(&mut self) -> R<()> {
        let m = self.k;
        let mut specials_seen = 0u8;
        for g in m.global_ids() {
            self.variable(g, &mut specials_seen)?;
        }
        for ifunc in [false, true] {
            for a in m.alias_ids() {
                if m.alias(a).is_ifunc() == ifunc {
                    self.indirect(a)?;
                }
            }
        }
        let funcs: Vec<k::FuncId> = m.functions().map(|(f, _)| f).collect();
        for f in funcs {
            self.function(f)?;
        }
        Ok(())
    }

    fn prefix(&mut self, dll: k::DllStorage) {
        if dll != k::DllStorage::Default {
            self.note("dll-storage");
        }
    }

    /// A global value's attachments: its `!exclude` returned, its `!dbg`
    /// nodes collected to `dbg.attached`.
    fn attachments(&mut self, md: &[(k::MdKindId, k::MetadataId)]) -> Option<k::MetadataId> {
        let mut exclude = None;
        self.dbg.attached.clear();
        for &(kind, node) in md {
            let name = self.k.md_kind_name(kind);
            match GlobalAttachment::of(name.as_bytes()) {
                GlobalAttachment::Debug => self.dbg.attached.push(node),
                GlobalAttachment::Dropped => {}
                GlobalAttachment::Exclude => {
                    exclude.get_or_insert(node);
                }
                GlobalAttachment::Unsupported => {
                    self.note(kit::global_attachment_construct(name.as_bytes()));
                }
            }
        }
        exclude
    }

    fn variable(&mut self, g: k::GlobalId, specials_seen: &mut u8) -> R<()> {
        let m = self.k;
        let v = m.global(g);
        self.prefix(v.dll_storage);
        let ty = self.ty(v.value_type())?;
        let declaration = v.initializer().is_none();
        if declaration && !matches!(v.linkage, k::Linkage::External | k::Linkage::ExternWeak) {
            return Err(Fallback::new("variable-without-initializer"));
        }
        let init = match v.initializer() {
            Some(init) => Some(self.constant(init)?),
            None => None,
        };
        if v.sanitizer.memtag {
            self.note("global-sanitize-memtag");
        }
        let exclude = self.attachments(v.metadata());
        if let Some(special) = SpecialGlobal::from_name(v.name().as_bytes()) {
            if *specials_seen & special.bit() != 0 {
                return Err(Fallback::new("special-redefined"));
            }
            *specials_seen |= special.bit();
            if special == SpecialGlobal::Annotations {
                if let Some(construct) =
                    kit::annotations_construct(v.section.as_deref().map(str::as_bytes))
                {
                    self.note(construct);
                }
            } else if let Some(init) = init {
                match kit::deliver_special(&mut self.module, special, init) {
                    Ok(()) => {}
                    Err(SpecialDeliveryError::Malformed(_)) => {
                        return Err(Fallback::new("special-malformed"));
                    }
                    Err(SpecialDeliveryError::Ir(e)) => return ir(Err(e)),
                }
            }
            self.flush(None);
            return Ok(());
        }
        let sym = self.globals[g.index()].ok_or_else(|| Fallback::new("symbol"))?;
        if v.name().starts_with("llvm.") {
            self.note(format!("special-global:{}", v.name()));
        }
        let comdat = match v.comdat {
            Some(c) => Some(self.comdats[c.index()].ok_or_else(|| Fallback::new("comdat"))?),
            None => None,
        };
        let desc = VariableDesc {
            linkage: linkage(v.linkage),
            visibility: visibility(v.visibility),
            dso_local: printed_dso_local(v.linkage, v.dso_local, v.visibility),
            unnamed_addr: unnamed_addr(v.unnamed_addr),
            section: v.section.as_deref().map(str::as_bytes),
            comdat,
            align: v.align,
            ty,
            constant: v.is_constant,
            tls: tls(v.thread_local),
            addr_space: v.addrspace,
            init,
        };
        ir(self.module.set_variable(sym, &desc))?;
        if let Some(node) = exclude {
            match m.md_node(node) {
                k::MdNode::Tuple { ops, .. } if ops.is_empty() => ir(self.module.exclude(sym))?,
                k::MdNode::Tuple { .. } => {}
                _ => self.note(kit::global_attachment_construct(b"exclude")),
            }
        }
        self.variable_dbg(sym)?;
        self.flush(Some(sym));
        Ok(())
    }

    fn indirect(&mut self, a: k::AliasId) -> R<()> {
        let m = self.k;
        let alias = m.alias(a);
        self.prefix(alias.dll_storage);
        let ty = self.ty(alias.value_type())?;
        let aliasee = alias.aliasee();
        let is_expr = matches!(
            m.const_of(aliasee),
            Some(
                k::ConstData::Gep { .. } | k::ConstData::Cast { .. } | k::ConstData::Binary { .. }
            )
        );
        if is_expr && m.const_type(aliasee) != k::TypeId::PTR {
            return Err(Fallback::new("alias-expr-addrspace"));
        }
        let target = self.constant(aliasee)?;
        let sym = self.aliases[a.index()].ok_or_else(|| Fallback::new("symbol"))?;
        let common = (
            linkage(alias.linkage),
            visibility(alias.visibility),
            printed_dso_local(alias.linkage, alias.dso_local, alias.visibility),
        );
        if alias.is_ifunc() {
            let Const::Addr { sym: resolver, addend: 0 } = self.module.consts.get(target) else {
                return Err(Fallback::new("ifunc-resolver"));
            };
            let desc = IFuncDesc {
                linkage: common.0,
                visibility: common.1,
                dso_local: common.2,
                unnamed_addr: UnnamedAddr::None,
                ty,
                resolver,
            };
            ir(self.module.set_ifunc(sym, &desc))?;
        } else {
            let desc = hod_ir::AliasDesc {
                linkage: common.0,
                visibility: common.1,
                dso_local: common.2,
                unnamed_addr: unnamed_addr(alias.unnamed_addr),
                tls: tls(alias.thread_local),
                ty,
                target,
            };
            ir(self.module.set_alias(sym, &desc))?;
        }
        self.flush(Some(sym));
        Ok(())
    }

    fn function(&mut self, fid: k::FuncId) -> R<()> {
        let m = self.k;
        let f = m.func(fid);
        let defined = !f.is_declaration();
        if !defined {
            self.attachments(f.metadata());
        }
        self.prefix(f.dll_storage);
        let cc = call_conv(f.cc)?;
        let ret_attrs = self.value_attrs(f.attrs.ret)?;
        let ty = self.ty(f.ty())?;
        let (_, params, _) =
            m.types.function_info(f.ty()).ok_or_else(|| Fallback::new("fn-type"))?;
        let mut param_attrs = Vec::with_capacity(params.len());
        for i in 0..params.len() {
            param_attrs.push(self.value_attrs(f.attrs.param(i))?);
        }
        while param_attrs.last().is_some_and(|a| a.is_none()) {
            param_attrs.pop();
        }
        let (attrs, group_align) = self.function_attrs(f.attrs.func, defined)?;
        let comdat = match f.comdat {
            Some(c) => Some(self.comdats[c.index()].ok_or_else(|| Fallback::new("comdat"))?),
            None => None,
        };
        if f.gc.is_some() {
            self.note("gc");
        }
        let mut personality = None;
        if let Some(p) = f.personality {
            let p = self.constant(p)?;
            match self.module.consts.get(p) {
                Const::Addr { sym, addend: 0 } => personality = Some(sym),
                _ => self.note("personality-not-a-symbol"),
            }
        }
        if defined {
            self.attachments(f.metadata());
        }
        let subprogram = self.function_dbg()?;
        let sym = self.funcs[fid.index()].ok_or_else(|| Fallback::new("symbol"))?;
        let desc = FunctionDesc {
            linkage: linkage(f.linkage),
            visibility: visibility(f.visibility),
            dso_local: printed_dso_local(f.linkage, f.dso_local, f.visibility),
            unnamed_addr: unnamed_addr(f.unnamed_addr),
            section: f.section.as_deref().map(str::as_bytes),
            comdat,
            align: group_align.or(f.align),
            ty,
            cc,
            ret_attrs,
            param_attrs: &param_attrs,
            attrs,
            personality,
            subprogram,
            addr_space: f.addrspace,
            has_body: defined,
        };
        ir(self.module.set_function(sym, &desc))?;
        if defined {
            self.defined.push((fid, sym));
        }
        self.flush(Some(sym));
        Ok(())
    }
}

/// The keyword an attribute other than a string attribute is written
/// with.
fn keyword(a: &k::Attr) -> &'static str {
    match a {
        k::Attr::Flag(kind) => kind.name(),
        k::Attr::ByRef(_) => "byref",
        k::Attr::ByVal(_) => "byval",
        k::Attr::ElementType(_) => "elementtype",
        k::Attr::InAlloca(_) => "inalloca",
        k::Attr::Preallocated(_) => "preallocated",
        k::Attr::SRet(_) => "sret",
        k::Attr::Align(_) => "align",
        k::Attr::AllocSize(..) => "allocsize",
        k::Attr::CapturesNone => "captures",
        k::Attr::Dereferenceable(_) => "dereferenceable",
        k::Attr::DereferenceableOrNull(_) => "dereferenceable_or_null",
        k::Attr::Memory(_) => "memory",
        k::Attr::NoFpClass(_) => "nofpclass",
        k::Attr::AlignStack(_) => "alignstack",
        k::Attr::UwTable(_) => "uwtable",
        k::Attr::VScaleRange(..) => "vscale_range",
        k::Attr::Str(..) => "",
    }
}

fn call_conv(cc: k::CallConv) -> R<CallConv> {
    CallConv::from_llvm(cc.0).map_err(|_| Fallback::new("calling-convention"))
}

fn linkage(l: k::Linkage) -> Linkage {
    match l {
        k::Linkage::External => Linkage::External,
        k::Linkage::AvailableExternally => Linkage::AvailableExternally,
        k::Linkage::LinkOnce => Linkage::LinkOnceAny,
        k::Linkage::LinkOnceOdr => Linkage::LinkOnceOdr,
        k::Linkage::Weak => Linkage::WeakAny,
        k::Linkage::WeakOdr => Linkage::WeakOdr,
        k::Linkage::Appending => Linkage::Appending,
        k::Linkage::Internal => Linkage::Internal,
        k::Linkage::Private => Linkage::Private,
        k::Linkage::ExternWeak => Linkage::ExternWeak,
        k::Linkage::Common => Linkage::Common,
    }
}

fn visibility(v: k::Visibility) -> Visibility {
    match v {
        k::Visibility::Default => Visibility::Default,
        k::Visibility::Hidden => Visibility::Hidden,
        k::Visibility::Protected => Visibility::Protected,
    }
}

/// `dso_local` as the module prints it: implied (not written) for a
/// local symbol and for one with a visibility other than default.
fn printed_dso_local(l: k::Linkage, dso: bool, v: k::Visibility) -> bool {
    let implicit = l.is_local() || (v != k::Visibility::Default && l != k::Linkage::ExternWeak);
    dso && !implicit
}

fn unnamed_addr(u: k::UnnamedAddr) -> UnnamedAddr {
    match u {
        k::UnnamedAddr::None => UnnamedAddr::None,
        k::UnnamedAddr::Local => UnnamedAddr::Local,
        k::UnnamedAddr::Global => UnnamedAddr::Global,
    }
}

fn tls(t: Option<k::TlsModel>) -> TlsModel {
    match t {
        None => TlsModel::None,
        Some(k::TlsModel::GeneralDynamic) => TlsModel::GeneralDynamic,
        Some(k::TlsModel::LocalDynamic) => TlsModel::LocalDynamic,
        Some(k::TlsModel::InitialExec) => TlsModel::InitialExec,
        Some(k::TlsModel::LocalExec) => TlsModel::LocalExec,
    }
}

/// `FlagValue` is part of the reader's module-flag mapping
/// ([`kit::module_flag`]); named here so a change of its shape in hod
/// fails to build here.
#[allow(dead_code, reason = "a compile-time check of the pinned API")]
fn _flag_value(v: FlagValue<'_>) -> FlagValue<'_> {
    v
}
