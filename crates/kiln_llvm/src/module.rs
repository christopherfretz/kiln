//! The module: globals, functions, constants, attributes, metadata
//! (`spec/llvm.md` §2).

use std::collections::HashMap;

use crate::attr::{Attr, AttrTable};
use crate::constant::{ConstData, ConstTable, IntValue, mask128};
use crate::error::{IrError, IrErrorKind, IrResult};
use crate::function::Function;
use crate::inst::{BinOp, CastOp, Flags, GepFlags, SyncScope};
use crate::metadata::{MdField, MdNode};
use crate::types::{TypeData, TypeId, TypeTable};
use crate::value::{
    AliasId, AsmId, AttrSetId, BlockId, ComdatId, ConstId, FuncId, GlobalId, MdKindId, MetadataId,
    Value, ValueRef,
};

/// Linkage types.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Default)]
#[allow(missing_docs)]
pub enum Linkage {
    #[default]
    External,
    AvailableExternally,
    LinkOnce,
    LinkOnceOdr,
    Weak,
    WeakOdr,
    Appending,
    Internal,
    Private,
    ExternWeak,
    Common,
}

impl Linkage {
    /// Keyword (empty for `external`).
    pub fn keyword(self) -> &'static str {
        match self {
            Linkage::External => "",
            Linkage::AvailableExternally => "available_externally",
            Linkage::LinkOnce => "linkonce",
            Linkage::LinkOnceOdr => "linkonce_odr",
            Linkage::Weak => "weak",
            Linkage::WeakOdr => "weak_odr",
            Linkage::Appending => "appending",
            Linkage::Internal => "internal",
            Linkage::Private => "private",
            Linkage::ExternWeak => "extern_weak",
            Linkage::Common => "common",
        }
    }

    /// `internal` or `private`.
    pub fn is_local(self) -> bool {
        matches!(self, Linkage::Internal | Linkage::Private)
    }
}

/// Symbol visibility.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Default)]
#[allow(missing_docs)]
pub enum Visibility {
    #[default]
    Default,
    Hidden,
    Protected,
}

/// DLL storage class.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Default)]
#[allow(missing_docs)]
pub enum DllStorage {
    #[default]
    Default,
    Import,
    Export,
}

/// `unnamed_addr` markers.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Default)]
pub enum UnnamedAddr {
    /// None.
    #[default]
    None,
    /// `local_unnamed_addr`.
    Local,
    /// `unnamed_addr`.
    Global,
}

/// Thread-local storage models.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum TlsModel {
    /// `thread_local`.
    GeneralDynamic,
    /// `thread_local(localdynamic)`.
    LocalDynamic,
    /// `thread_local(initialexec)`.
    InitialExec,
    /// `thread_local(localexec)`.
    LocalExec,
}

/// Comdat selection kinds.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Default)]
#[allow(missing_docs)]
pub enum ComdatKind {
    #[default]
    Any,
    ExactMatch,
    Largest,
    NoDeduplicate,
    SameSize,
}

/// A comdat (`$name = comdat any`).
#[derive(Clone, Debug)]
pub struct Comdat {
    pub(crate) name: Box<str>,
    /// Selection kind.
    pub kind: ComdatKind,
}

/// A global variable.
#[derive(Clone, Debug)]
pub struct GlobalVar {
    pub(crate) name: Box<str>,
    pub(crate) value_type: TypeId,
    pub(crate) init: Option<Value>,
    /// `constant` instead of `global`.
    pub is_constant: bool,
    /// Linkage.
    pub linkage: Linkage,
    /// Visibility.
    pub visibility: Visibility,
    /// DLL storage class.
    pub dll_storage: DllStorage,
    /// `dso_local`.
    pub dso_local: bool,
    /// Thread-local model.
    pub thread_local: Option<TlsModel>,
    /// `unnamed_addr` / `local_unnamed_addr`.
    pub unnamed_addr: UnnamedAddr,
    /// Address space.
    pub addrspace: u32,
    /// `externally_initialized`.
    pub externally_initialized: bool,
    /// `section "..."`.
    pub section: Option<Box<str>>,
    /// `comdat`.
    pub comdat: Option<ComdatId>,
    /// `align N`.
    pub align: Option<u64>,
    pub(crate) md: Vec<(MdKindId, MetadataId)>,
}

impl GlobalVar {
    /// Name.
    pub fn name(&self) -> &str {
        &self.name
    }
    /// The type of the initializer / stored value.
    pub fn value_type(&self) -> TypeId {
        self.value_type
    }
    /// The initializer (none for a declaration).
    pub fn initializer(&self) -> Option<Value> {
        self.init
    }
}

/// A global alias.
#[derive(Clone, Debug)]
pub struct Alias {
    pub(crate) name: Box<str>,
    pub(crate) value_type: TypeId,
    pub(crate) aliasee: Value,
    /// Linkage.
    pub linkage: Linkage,
    /// Visibility.
    pub visibility: Visibility,
    /// DLL storage class.
    pub dll_storage: DllStorage,
    /// `dso_local`.
    pub dso_local: bool,
    /// Thread-local model.
    pub thread_local: Option<TlsModel>,
    /// `unnamed_addr`.
    pub unnamed_addr: UnnamedAddr,
    /// Address space of the alias pointer.
    pub addrspace: u32,
}

impl Alias {
    /// Name.
    pub fn name(&self) -> &str {
        &self.name
    }
}

/// An inline asm callee.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
#[allow(clippy::struct_excessive_bools)]
pub struct InlineAsm {
    /// The function type of the call.
    pub fn_ty: TypeId,
    /// Assembly text.
    pub asm: Box<str>,
    /// Constraint string.
    pub constraints: Box<str>,
    /// `sideeffect`.
    pub side_effect: bool,
    /// `alignstack`.
    pub align_stack: bool,
    /// `inteldialect`.
    pub intel_dialect: bool,
    /// `unwind`.
    pub can_throw: bool,
}

/// Target triple and data layout pair.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct TargetSpec {
    /// `target triple`.
    pub triple: &'static str,
    /// `target datalayout`.
    pub data_layout: &'static str,
}

/// clang 19's `aarch64-unknown-linux-gnu` triple and data layout.
pub const AARCH64_LINUX_GNU: TargetSpec = TargetSpec {
    triple: "aarch64-unknown-linux-gnu",
    data_layout: "e-m:e-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32",
};

/// clang 19's `x86_64-unknown-linux-gnu` triple and data layout.
pub const X86_64_LINUX_GNU: TargetSpec = TargetSpec {
    triple: "x86_64-unknown-linux-gnu",
    data_layout: "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128",
};

/// Metadata kinds with fixed ids in LLVM 19 (`LLVMContext` order); they
/// are registered by [`Module::new`] so attachment order matches LLVM's.
const FIXED_MD_KINDS: [&str; 40] = [
    "dbg",
    "tbaa",
    "prof",
    "fpmath",
    "range",
    "tbaa.struct",
    "invariant.load",
    "alias.scope",
    "noalias",
    "nontemporal",
    "llvm.mem.parallel_loop_access",
    "nonnull",
    "dereferenceable",
    "dereferenceable_or_null",
    "make.implicit",
    "unpredictable",
    "invariant.group",
    "align",
    "llvm.loop",
    "type",
    "section_prefix",
    "absolute_symbol",
    "associated",
    "callees",
    "irr_loop",
    "llvm.access.group",
    "callback",
    "llvm.preserve.access.index",
    "vcall_visibility",
    "noundef",
    "annotation",
    "nosanitize",
    "func_sanitize",
    "exclude",
    "memprof",
    "callsite",
    "kcfi_type",
    "pcsections",
    "DIAssignID",
    "coro.outside.frame",
];

/// A module: the unit printed as one `.ll` file.
#[derive(Debug)]
pub struct Module {
    /// `; ModuleID = '...'` (defaults to the source file name).
    pub module_id: String,
    /// `source_filename = "..."`.
    pub source_filename: String,
    /// `target datalayout` (empty: not printed).
    pub data_layout: String,
    /// `target triple` (empty: not printed).
    pub target_triple: String,
    /// `module asm "..."` lines.
    pub module_asm: Vec<String>,
    /// Types.
    pub types: TypeTable,
    pub(crate) consts: ConstTable,
    pub(crate) funcs: Vec<Function>,
    pub(crate) globals: Vec<GlobalVar>,
    pub(crate) aliases: Vec<Alias>,
    pub(crate) comdats: Vec<Comdat>,
    comdat_names: HashMap<Box<str>, ComdatId>,
    pub(crate) asms: Vec<InlineAsm>,
    asm_map: HashMap<InlineAsm, AsmId>,
    pub(crate) attrs: AttrTable,
    pub(crate) md: Vec<MdNode>,
    md_map: HashMap<MdNode, MetadataId>,
    pub(crate) named_md: Vec<(Box<str>, Vec<MetadataId>)>,
    pub(crate) md_kinds: Vec<Box<str>>,
    pub(crate) sync_scopes: Vec<Box<str>>,
    gnames: HashMap<Box<str>, Value>,
    last_unique: u32,
}

impl Module {
    /// An empty module without target information.
    pub fn new(source_filename: &str) -> Module {
        Module {
            module_id: source_filename.to_owned(),
            source_filename: source_filename.to_owned(),
            data_layout: String::new(),
            target_triple: String::new(),
            module_asm: Vec::new(),
            types: TypeTable::new(),
            consts: ConstTable::default(),
            funcs: Vec::new(),
            globals: Vec::new(),
            aliases: Vec::new(),
            comdats: Vec::new(),
            comdat_names: HashMap::new(),
            asms: Vec::new(),
            asm_map: HashMap::new(),
            attrs: AttrTable::new(),
            md: Vec::new(),
            md_map: HashMap::new(),
            named_md: Vec::new(),
            md_kinds: FIXED_MD_KINDS.iter().map(|&k| Box::from(k)).collect(),
            sync_scopes: vec![Box::from(""), Box::from("singlethread")],
            gnames: HashMap::new(),
            last_unique: 0,
        }
    }

    /// An empty module for a target.
    pub fn with_target(source_filename: &str, target: TargetSpec) -> Module {
        let mut m = Module::new(source_filename);
        target.triple.clone_into(&mut m.target_triple);
        target.data_layout.clone_into(&mut m.data_layout);
        m
    }

    // ---- global values --------------------------------------------------

    fn unique_global_name(&mut self, name: &str) -> Box<str> {
        if name.is_empty() || !self.gnames.contains_key(name) {
            return name.into();
        }
        loop {
            self.last_unique += 1;
            let cand = format!("{name}.{}", self.last_unique);
            if !self.gnames.contains_key(cand.as_str()) {
                return cand.into_boxed_str();
            }
        }
    }

    fn register_global_name(&mut self, name: &str, v: Value) {
        if !name.is_empty() {
            self.gnames.insert(name.into(), v);
        }
    }

    /// Look up a function, variable or alias by name.
    pub fn lookup(&self, name: &str) -> Option<Value> {
        self.gnames.get(name).copied()
    }

    /// Add a function declaration of type `fn_ty`; the name is uniqued
    /// (`name.N`) if taken. Errors unless `fn_ty` is a function type.
    pub fn add_function(&mut self, name: &str, fn_ty: TypeId) -> IrResult<FuncId> {
        let Some((ret, params, varargs)) = self.types.function_info(fn_ty) else {
            return Err(IrError::new(
                IrErrorKind::InvalidType,
                format!("function type expected, found {}", self.types.display(fn_ty)),
            ));
        };
        let params: Vec<TypeId> = params.to_vec();
        let name = self.unique_global_name(name);
        let id = FuncId(self.funcs.len() as u32);
        self.register_global_name(&name, Value::func(id));
        self.funcs.push(Function::new(name, fn_ty, ret, &params, varargs));
        Ok(id)
    }

    /// The function named `name`, or a new declaration of type `fn_ty`.
    pub fn get_or_insert_function(&mut self, name: &str, fn_ty: TypeId) -> IrResult<FuncId> {
        if let Some(ValueRef::Func(f)) = self.lookup(name).map(Value::get) {
            return Ok(f);
        }
        self.add_function(name, fn_ty)
    }

    /// A function by name.
    pub fn get_function(&self, name: &str) -> Option<FuncId> {
        match self.lookup(name)?.get() {
            ValueRef::Func(f) => Some(f),
            _ => None,
        }
    }

    /// Function by id.
    pub fn func(&self, f: FuncId) -> &Function {
        &self.funcs[f.index()]
    }

    /// Function by id, mutable.
    pub fn func_mut(&mut self, f: FuncId) -> &mut Function {
        &mut self.funcs[f.index()]
    }

    /// Functions in creation (print) order.
    pub fn functions(&self) -> impl Iterator<Item = (FuncId, &Function)> {
        self.funcs.iter().enumerate().map(|(i, f)| (FuncId(i as u32), f))
    }

    /// Run [`crate::verify_function`] over every function, naming the
    /// offending function in the error message.
    ///
    /// # Errors
    ///
    /// The first violation, in function order.
    pub fn verify(&self) -> IrResult<()> {
        for f in &self.funcs {
            crate::verify::verify_function(f).map_err(|e| {
                IrError::new(e.kind, format!("in function @{}: {}", f.name(), e.message))
            })?;
        }
        Ok(())
    }

    /// Add a global variable declaration (`external global T`); set an
    /// initializer with [`Module::set_initializer`] to define it.
    pub fn add_global(&mut self, name: &str, value_type: TypeId) -> GlobalId {
        let name = self.unique_global_name(name);
        let id = GlobalId(self.globals.len() as u32);
        self.register_global_name(&name, Value::global(id));
        self.globals.push(GlobalVar {
            name,
            value_type,
            init: None,
            is_constant: false,
            linkage: Linkage::External,
            visibility: Visibility::Default,
            dll_storage: DllStorage::Default,
            dso_local: false,
            thread_local: None,
            unnamed_addr: UnnamedAddr::None,
            addrspace: 0,
            externally_initialized: false,
            section: None,
            comdat: None,
            align: None,
            md: Vec::new(),
        });
        id
    }

    /// A global variable by name.
    pub fn get_global(&self, name: &str) -> Option<GlobalId> {
        match self.lookup(name)?.get() {
            ValueRef::Global(g) => Some(g),
            _ => None,
        }
    }

    /// Global by id.
    pub fn global(&self, g: GlobalId) -> &GlobalVar {
        &self.globals[g.index()]
    }

    /// Global by id, mutable.
    pub fn global_mut(&mut self, g: GlobalId) -> &mut GlobalVar {
        &mut self.globals[g.index()]
    }

    /// Set (or clear) a global's initializer; it must be a constant of the
    /// global's value type.
    pub fn set_initializer(&mut self, g: GlobalId, init: Option<Value>) -> IrResult<()> {
        if let Some(v) = init {
            let want = self.globals[g.index()].value_type;
            self.expect_const_type(v, want, "global initializer")?;
        }
        self.globals[g.index()].init = init;
        Ok(())
    }

    /// Change a global's value type in place, clearing its initializer
    /// (clang replaces a static local whose initializer has a different
    /// IR type by a new global at the same position; pointers are opaque,
    /// so existing uses stay valid).
    pub fn set_global_value_type(&mut self, g: GlobalId, value_type: TypeId) {
        let gv = &mut self.globals[g.index()];
        gv.value_type = value_type;
        gv.init = None;
    }

    /// Attach metadata to a global variable (`, !dbg !N`).
    pub fn add_global_metadata(&mut self, g: GlobalId, kind: MdKindId, md: MetadataId) {
        set_attachment(&mut self.globals[g.index()].md, kind, md);
    }

    /// Add an alias `@name = alias value_type, ptr aliasee`.
    pub fn add_alias(
        &mut self,
        name: &str,
        value_type: TypeId,
        aliasee: Value,
    ) -> IrResult<AliasId> {
        if !aliasee.is_constant() || !self.types.is_ptr(self.const_type(aliasee)) {
            return Err(IrError::new(IrErrorKind::BadGlobal, "aliasee must be a pointer constant"));
        }
        let name = self.unique_global_name(name);
        let id = AliasId(self.aliases.len() as u32);
        self.register_global_name(&name, Value::alias(id));
        let addrspace = self.types.ptr_addrspace(self.const_type(aliasee)).unwrap_or(0);
        self.aliases.push(Alias {
            name,
            value_type,
            aliasee,
            linkage: Linkage::External,
            visibility: Visibility::Default,
            dll_storage: DllStorage::Default,
            dso_local: false,
            thread_local: None,
            unnamed_addr: UnnamedAddr::None,
            addrspace,
        });
        Ok(id)
    }

    /// Alias by id.
    pub fn alias(&self, a: AliasId) -> &Alias {
        &self.aliases[a.index()]
    }

    /// Alias by id, mutable.
    pub fn alias_mut(&mut self, a: AliasId) -> &mut Alias {
        &mut self.aliases[a.index()]
    }

    /// The comdat named `name` (created with `kind` if new).
    pub fn comdat(&mut self, name: &str, kind: ComdatKind) -> ComdatId {
        if let Some(&c) = self.comdat_names.get(name) {
            return c;
        }
        let id = ComdatId(self.comdats.len() as u32);
        self.comdats.push(Comdat { name: name.into(), kind });
        self.comdat_names.insert(name.into(), id);
        id
    }

    /// Comdat name.
    pub fn comdat_name(&self, c: ComdatId) -> &str {
        &self.comdats[c.index()].name
    }

    /// An inline asm callee (interned).
    pub fn inline_asm(&mut self, asm: InlineAsm) -> IrResult<Value> {
        if !self.types.is_function(asm.fn_ty) {
            return Err(IrError::new(IrErrorKind::InvalidType, "inline asm needs a function type"));
        }
        if let Some(&id) = self.asm_map.get(&asm) {
            return Ok(Value::asm(id));
        }
        let id = AsmId(self.asms.len() as u32);
        self.asms.push(asm.clone());
        self.asm_map.insert(asm, id);
        Ok(Value::asm(id))
    }

    /// Inline asm by id.
    pub fn asm(&self, a: AsmId) -> &InlineAsm {
        &self.asms[a.index()]
    }

    // ---- attributes -----------------------------------------------------

    /// Intern an attribute set (canonicalized: later duplicates of a key
    /// win, then sorted).
    pub fn attr_set(&mut self, attrs: &[Attr]) -> AttrSetId {
        self.attrs.intern(attrs)
    }

    /// The attributes of a set.
    pub fn attrs_of(&self, s: AttrSetId) -> &[Attr] {
        self.attrs.get(s)
    }

    // ---- value types ----------------------------------------------------

    /// The type of a module-level value (constant, global value, inline
    /// asm, metadata). Function-local values have no module-level type;
    /// use [`Module::value_type`].
    pub fn const_type(&self, v: Value) -> TypeId {
        match v.get() {
            ValueRef::Const(c) => self.consts.get(c).ty(),
            ValueRef::Func(_) | ValueRef::Asm(_) => TypeId::PTR,
            ValueRef::Global(g) => self.ptr_type_in(self.globals[g.index()].addrspace),
            ValueRef::Alias(a) => self.ptr_type_in(self.aliases[a.index()].addrspace),
            ValueRef::Metadata(_) => TypeId::METADATA,
            ValueRef::Block(_) => TypeId::LABEL,
            ValueRef::Inst(_) | ValueRef::Arg(_) => TypeId::VOID,
        }
    }

    fn ptr_type_in(&self, addrspace: u32) -> TypeId {
        if addrspace == 0 {
            return TypeId::PTR;
        }
        // Pointer types for every address space in use are interned when
        // the global/alias is created or its address space is set; fall
        // back to a lookup.
        self.types
            .lookup(&TypeData::Ptr(addrspace))
            .unwrap_or_else(|| unreachable!("pointer type for addrspace {addrspace} not interned"))
    }

    /// The type of any value used in function `f`.
    pub fn value_type(&self, f: FuncId, v: Value) -> TypeId {
        if v.is_local() {
            self.funcs[f.index()].local_type(v).unwrap_or(TypeId::VOID)
        } else {
            self.const_type(v)
        }
    }

    /// Constant data by id.
    pub fn const_data(&self, c: ConstId) -> &ConstData {
        self.consts.get(c)
    }

    /// The data of a constant value, if it is an interned constant.
    pub fn const_of(&self, v: Value) -> Option<&ConstData> {
        v.as_const().map(|c| self.consts.get(c))
    }

    pub(crate) fn expect_const_type(&self, v: Value, want: TypeId, what: &str) -> IrResult<()> {
        if !v.is_constant() {
            return Err(IrError::new(
                IrErrorKind::InvalidOperand,
                format!("{what} must be a constant"),
            ));
        }
        let got = self.const_type(v);
        if got != want {
            return Err(IrError::new(
                IrErrorKind::TypeMismatch,
                format!(
                    "{what}: expected {}, found {}",
                    self.types.display(want),
                    self.types.display(got)
                ),
            ));
        }
        Ok(())
    }

    // ---- constants ------------------------------------------------------

    /// Integer constant of type `ty` from the low bits of `v` (so negative
    /// values work for any width ≤ 128; wider types sign-extend `v`).
    pub fn const_int(&mut self, ty: TypeId, v: i128) -> Value {
        let bits = self.types.int_width(ty).unwrap_or_else(|| {
            unreachable!("const_int on non-integer type {}", self.types.display(ty))
        });
        let value = if bits <= 128 {
            IntValue::Small(mask128(v as u128, bits))
        } else {
            let n = bits.div_ceil(64) as usize;
            let mut w = vec![if v < 0 { u64::MAX } else { 0 }; n];
            w[0] = v as u64;
            w[1] = (v >> 64) as u64;
            mask_words(&mut w, bits);
            IntValue::Big(w.into_boxed_slice())
        };
        self.consts.intern(ConstData::Int { ty, value })
    }

    /// Integer constant from little-endian words (any width).
    pub fn const_int_words(&mut self, ty: TypeId, words: &[u64]) -> Value {
        let bits = self.types.int_width(ty).unwrap_or_else(|| unreachable!("non-integer type"));
        let n = bits.div_ceil(64) as usize;
        let mut w = vec![0u64; n];
        for (d, s) in w.iter_mut().zip(words) {
            *d = *s;
        }
        mask_words(&mut w, bits);
        let value = if bits <= 128 {
            IntValue::Small(u128::from(w[0]) | (u128::from(w.get(1).copied().unwrap_or(0)) << 64))
        } else {
            IntValue::Big(w.into_boxed_slice())
        };
        self.consts.intern(ConstData::Int { ty, value })
    }

    /// `i1 true` / `i1 false`.
    pub fn const_bool(&mut self, b: bool) -> Value {
        self.const_int(TypeId::I1, i128::from(b))
    }

    /// Floating-point constant from its raw encoding.
    pub fn const_float_bits(&mut self, ty: TypeId, bits: u128) -> Value {
        debug_assert!(self.types.is_float(ty), "const_float_bits on non-float type");
        let w = self.types.float_width(ty).unwrap_or(128);
        self.consts.intern(ConstData::Float { ty, bits: mask128(bits, w) })
    }

    /// Floating-point constant of type `ty` with the value `v` (rounded to
    /// nearest-even for narrower types, exact for wider ones).
    pub fn const_float(&mut self, ty: TypeId, v: f64) -> Value {
        let bits: u128 = match self.types.data(ty) {
            TypeData::Double => u128::from(v.to_bits()),
            TypeData::Float => u128::from((v as f32).to_bits()),
            TypeData::Half => u128::from(crate::float::f64_to_minifloat(v, 5, 10)),
            TypeData::BFloat => u128::from(crate::float::f64_to_minifloat(v, 8, 7)),
            TypeData::Fp128 => crate::float::f64_to_fp128(v),
            TypeData::X86Fp80 => crate::float::f64_to_x87(v),
            _ => unreachable!("const_float on non-float type {}", self.types.display(ty)),
        };
        self.consts.intern(ConstData::Float { ty, bits })
    }

    /// `null` of a pointer type.
    pub fn const_null(&mut self, ty: TypeId) -> Value {
        debug_assert!(self.types.is_ptr(ty), "const_null on non-pointer type");
        self.consts.intern(ConstData::Null(ty))
    }

    /// The zero value of `ty`: `0`, `0.0`, `null`, `none` or
    /// `zeroinitializer`.
    pub fn const_zero(&mut self, ty: TypeId) -> Value {
        match self.types.data(ty) {
            TypeData::Int(_) => self.const_int(ty, 0),
            TypeData::Ptr(_) => self.const_null(ty),
            TypeData::Token => self.consts.intern(ConstData::NoneToken),
            _ if self.types.is_float(ty) => self.const_float_bits(ty, 0),
            _ => self.consts.intern(ConstData::Zero(ty)),
        }
    }

    /// `undef` of `ty`.
    pub fn const_undef(&mut self, ty: TypeId) -> Value {
        self.consts.intern(ConstData::Undef(ty))
    }

    /// `poison` of `ty`.
    pub fn const_poison(&mut self, ty: TypeId) -> Value {
        self.consts.intern(ConstData::Poison(ty))
    }

    /// `none` (token).
    pub fn const_none_token(&mut self) -> Value {
        self.consts.intern(ConstData::NoneToken)
    }

    fn aggregate_canon(&self, elems: &[Value]) -> Option<u8> {
        // 0 = all zero (or empty), 1 = all undef, 2 = all poison.
        if elems.is_empty() {
            return Some(0);
        }
        let kind = |v: Value| -> Option<u8> {
            match self.const_of(v)? {
                ConstData::Undef(_) => Some(1),
                ConstData::Poison(_) => Some(2),
                d if d.is_zero() => Some(0),
                _ => None,
            }
        };
        let first = kind(*elems.first()?)?;
        elems[1..].iter().all(|&e| kind(e) == Some(first)).then_some(first)
    }

    fn canon_aggregate(
        &mut self,
        ty: TypeId,
        elems: &[Value],
        make: impl FnOnce(TypeId, Box<[Value]>) -> ConstData,
    ) -> Value {
        match self.aggregate_canon(elems) {
            Some(0) => self.consts.intern(ConstData::Zero(ty)),
            Some(1) => self.consts.intern(ConstData::Undef(ty)),
            Some(2) => self.consts.intern(ConstData::Poison(ty)),
            _ => self.consts.intern(make(ty, elems.into())),
        }
    }

    fn check_const_elems(
        &self,
        elems: &[Value],
        tys: &mut dyn Iterator<Item = TypeId>,
    ) -> IrResult<()> {
        for (i, &e) in elems.iter().enumerate() {
            let want = tys.next().ok_or_else(|| {
                IrError::new(IrErrorKind::TypeMismatch, "too many aggregate elements")
            })?;
            self.expect_const_type(e, want, &format!("aggregate element {i}"))?;
        }
        Ok(())
    }

    /// `[N x elem] [...]`. An array of `i8` integers becomes a byte string.
    pub fn const_array(&mut self, elem: TypeId, elems: &[Value]) -> IrResult<Value> {
        self.check_const_elems(elems, &mut std::iter::repeat(elem))?;
        let ty = self.types.array(elem, elems.len() as u64);
        if elem == TypeId::I8 && !elems.is_empty() {
            let bytes: Option<Vec<u8>> = elems
                .iter()
                .map(|&e| match self.const_of(e)? {
                    ConstData::Int { value: IntValue::Small(v), .. } => Some(*v as u8),
                    _ => None,
                })
                .collect();
            if let Some(b) = bytes {
                return Ok(self.const_bytes_typed(ty, &b));
            }
        }
        Ok(self.canon_aggregate(ty, elems, |ty, elems| ConstData::Array { ty, elems }))
    }

    /// A byte string `[N x i8] c"..."` (no terminator added).
    pub fn const_bytes(&mut self, bytes: &[u8]) -> Value {
        let ty = self.types.array(TypeId::I8, bytes.len() as u64);
        self.const_bytes_typed(ty, bytes)
    }

    /// A C string: `bytes` plus a NUL terminator.
    pub fn const_cstring(&mut self, s: &[u8]) -> Value {
        let mut b = s.to_vec();
        b.push(0);
        self.const_bytes(&b)
    }

    fn const_bytes_typed(&mut self, ty: TypeId, bytes: &[u8]) -> Value {
        if bytes.iter().all(|&b| b == 0) {
            return self.consts.intern(ConstData::Zero(ty));
        }
        self.consts.intern(ConstData::Bytes { ty, bytes: bytes.into() })
    }

    /// A struct constant of (literal or identified) struct type `ty`.
    pub fn const_struct(&mut self, ty: TypeId, elems: &[Value]) -> IrResult<Value> {
        let Some(fields) = self.types.struct_fields(ty) else {
            return Err(IrError::new(
                IrErrorKind::InvalidType,
                "const_struct needs a struct type with a body",
            ));
        };
        if fields.len() != elems.len() {
            return Err(IrError::new(
                IrErrorKind::TypeMismatch,
                format!("struct has {} fields, {} given", fields.len(), elems.len()),
            ));
        }
        let fields = fields.to_vec();
        self.check_const_elems(elems, &mut fields.into_iter())?;
        Ok(self.canon_aggregate(ty, elems, |ty, elems| ConstData::Struct { ty, elems }))
    }

    /// A literal-struct constant whose type is formed from the elements.
    pub fn const_anon_struct(&mut self, elems: &[Value], packed: bool) -> IrResult<Value> {
        for &e in elems {
            if !e.is_constant() {
                return Err(IrError::new(
                    IrErrorKind::InvalidOperand,
                    "struct element must be a constant",
                ));
            }
        }
        let tys: Vec<TypeId> = elems.iter().map(|&e| self.const_type(e)).collect();
        let ty = self.types.struct_literal(&tys, packed);
        self.const_struct(ty, elems)
    }

    /// A vector constant `<N x T> <...>` (non-empty; elements share a type).
    pub fn const_vector(&mut self, elems: &[Value]) -> IrResult<Value> {
        let Some(&first) = elems.first() else {
            return Err(IrError::new(IrErrorKind::InvalidOperand, "empty vector constant"));
        };
        if !first.is_constant() {
            return Err(IrError::new(
                IrErrorKind::InvalidOperand,
                "vector element must be a constant",
            ));
        }
        let et = self.const_type(first);
        self.check_const_elems(elems, &mut std::iter::repeat(et))?;
        if !(self.types.is_int(et) || self.types.is_float(et) || self.types.is_ptr(et)) {
            return Err(IrError::new(IrErrorKind::InvalidType, "invalid vector element type"));
        }
        let ty = self.types.vector(et, elems.len() as u32);
        Ok(self.canon_aggregate(ty, elems, |ty, elems| ConstData::Vector { ty, elems }))
    }

    /// Constant `getelementptr` expression.
    pub fn const_gep(
        &mut self,
        source: TypeId,
        base: Value,
        indices: &[Value],
        flags: GepFlags,
        inrange: Option<(i64, i64)>,
    ) -> IrResult<Value> {
        if !base.is_constant() || indices.iter().any(|i| !i.is_constant()) {
            return Err(IrError::new(
                IrErrorKind::InvalidOperand,
                "constant GEP operands must be constants",
            ));
        }
        let base_ty = self.const_type(base);
        let idx_tys: Vec<TypeId> = indices.iter().map(|&i| self.const_type(i)).collect();
        let idx_consts: Vec<Option<u64>> = indices.iter().map(|&i| self.const_index(i)).collect();
        let ty = crate::builder::gep_result_type(
            &mut self.types,
            source,
            base_ty,
            &idx_tys,
            &idx_consts,
        )?;
        let mut ops = Vec::with_capacity(indices.len() + 1);
        ops.push(base);
        ops.extend_from_slice(indices);
        Ok(self.consts.intern(ConstData::Gep { ty, source, flags, inrange, ops: ops.into() }))
    }

    /// Constant cast expression (`trunc`, `ptrtoint`, `inttoptr`,
    /// `bitcast`, `addrspacecast` — the casts LLVM 19 still folds into
    /// constant expressions).
    pub fn const_cast(&mut self, op: CastOp, v: Value, ty: TypeId) -> IrResult<Value> {
        if !matches!(
            op,
            CastOp::Trunc
                | CastOp::PtrToInt
                | CastOp::IntToPtr
                | CastOp::BitCast
                | CastOp::AddrSpaceCast
        ) {
            return Err(IrError::new(
                IrErrorKind::InvalidOperand,
                format!("{} is not a constant expression in LLVM 19", op.name()),
            ));
        }
        if !v.is_constant() {
            return Err(IrError::new(
                IrErrorKind::InvalidOperand,
                "cast operand must be a constant",
            ));
        }
        let from = self.const_type(v);
        crate::builder::check_cast(&self.types, op, from, ty)?;
        Ok(self.consts.intern(ConstData::Cast { op, val: v, ty }))
    }

    /// Constant binary expression (`add`, `sub`, `mul`, `xor`).
    pub fn const_binop(
        &mut self,
        op: BinOp,
        flags: Flags,
        lhs: Value,
        rhs: Value,
    ) -> IrResult<Value> {
        if !matches!(op, BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Xor) {
            return Err(IrError::new(
                IrErrorKind::InvalidOperand,
                format!("{} is not a constant expression in LLVM 19", op.name()),
            ));
        }
        if !lhs.is_constant() || !rhs.is_constant() {
            return Err(IrError::new(IrErrorKind::InvalidOperand, "operands must be constants"));
        }
        let ty = self.const_type(lhs);
        if self.const_type(rhs) != ty || !self.types.is_int_or_int_vector(ty) {
            return Err(IrError::new(IrErrorKind::TypeMismatch, "constant binop operand types"));
        }
        if !op.allowed_flags().contains(flags) {
            return Err(IrError::new(IrErrorKind::InvalidFlags, "invalid flags on constant binop"));
        }
        Ok(self.consts.intern(ConstData::Binary { op, flags, lhs, rhs, ty }))
    }

    /// `blockaddress(@f, %bb)`.
    pub fn const_blockaddress(&mut self, f: FuncId, bb: BlockId) -> Value {
        let ty = TypeId::PTR;
        self.consts.intern(ConstData::BlockAddress { func: f, block: bb, ty })
    }

    /// `dso_local_equivalent @f`.
    pub fn const_dso_local_equivalent(&mut self, f: Value) -> Value {
        self.consts.intern(ConstData::DsoLocalEquivalent(f))
    }

    /// `no_cfi @f`.
    pub fn const_no_cfi(&mut self, f: Value) -> Value {
        self.consts.intern(ConstData::NoCfi(f))
    }

    /// Zero-extended value of a constant integer (≤ 64 bits significant).
    pub(crate) fn const_index(&self, v: Value) -> Option<u64> {
        match self.const_of(v)? {
            ConstData::Int { value: IntValue::Small(x), .. } => u64::try_from(*x).ok(),
            ConstData::Int { value: IntValue::Big(w), .. } => {
                w[1..].iter().all(|&x| x == 0).then_some(w[0])
            }
            _ => None,
        }
    }

    /// Number of interned constants.
    pub fn num_consts(&self) -> usize {
        self.consts.len()
    }

    // ---- metadata -------------------------------------------------------

    fn md_intern(&mut self, n: MdNode) -> MetadataId {
        if n.is_distinct() {
            let id = MetadataId(self.md.len() as u32);
            self.md.push(n);
            return id;
        }
        if let Some(&id) = self.md_map.get(&n) {
            return id;
        }
        let id = MetadataId(self.md.len() as u32);
        self.md.push(n.clone());
        self.md_map.insert(n, id);
        id
    }

    /// `!"s"`.
    pub fn md_string(&mut self, s: &[u8]) -> MetadataId {
        self.md_intern(MdNode::String(s.into()))
    }

    /// A constant as metadata (`i32 1`, `ptr @g`).
    pub fn md_const(&mut self, v: Value) -> IrResult<MetadataId> {
        if !v.is_constant() {
            return Err(IrError::new(IrErrorKind::BadMetadata, "md_const needs a constant"));
        }
        let ty = self.const_type(v);
        Ok(self.md_intern(MdNode::Value(ty, v)))
    }

    /// A function-local value as metadata (`metadata ptr %x` call
    /// arguments); `ty` must be the value's type in its function.
    pub fn md_local(&mut self, ty: TypeId, v: Value) -> MetadataId {
        self.md_intern(MdNode::Value(ty, v))
    }

    /// `!{...}` (uniqued).
    pub fn md_tuple(&mut self, ops: &[Option<MetadataId>]) -> MetadataId {
        self.md_intern(MdNode::Tuple { distinct: false, ops: ops.into() })
    }

    /// `distinct !{...}`.
    pub fn md_distinct_tuple(&mut self, ops: &[Option<MetadataId>]) -> MetadataId {
        self.md_intern(MdNode::Tuple { distinct: true, ops: ops.into() })
    }

    /// A specialized node `![distinct ]!Name(fields)`.
    pub fn md_specialized(
        &mut self,
        name: &str,
        distinct: bool,
        fields: Vec<(Box<str>, MdField)>,
    ) -> MetadataId {
        self.md_intern(MdNode::Specialized { distinct, name: name.into(), fields: fields.into() })
    }

    /// A metadata node by id.
    pub fn md_node(&self, m: MetadataId) -> &MdNode {
        &self.md[m.index()]
    }

    /// Replace an operand of a tuple (for building cycles such as
    /// self-referential loop metadata). The node must be `distinct`.
    pub fn md_set_operand(
        &mut self,
        node: MetadataId,
        i: usize,
        op: Option<MetadataId>,
    ) -> IrResult<()> {
        match &mut self.md[node.index()] {
            MdNode::Tuple { distinct: true, ops } if i < ops.len() => {
                ops[i] = op;
                Ok(())
            }
            _ => {
                Err(IrError::new(IrErrorKind::BadMetadata, "md_set_operand needs a distinct tuple"))
            }
        }
    }

    /// Append operands to named metadata `!name = !{...}` (created if new).
    pub fn add_named_metadata(&mut self, name: &str, ops: &[MetadataId]) {
        if let Some(e) = self.named_md.iter_mut().find(|e| &*e.0 == name) {
            e.1.extend_from_slice(ops);
        } else {
            self.named_md.push((name.into(), ops.to_vec()));
        }
    }

    /// Add `!{i32 behavior, !"key", value}` to `!llvm.module.flags`.
    pub fn add_module_flag(&mut self, behavior: u32, key: &str, value: MetadataId) {
        let b = self.const_int(TypeId::I32, i128::from(behavior));
        let b = self.md_intern(MdNode::Value(TypeId::I32, b));
        let k = self.md_string(key.as_bytes());
        let n = self.md_tuple(&[Some(b), Some(k), Some(value)]);
        self.add_named_metadata("llvm.module.flags", &[n]);
    }

    /// [`Module::add_module_flag`] with an `i32` value.
    pub fn add_module_flag_i32(&mut self, behavior: u32, key: &str, value: i32) {
        let v = self.const_int(TypeId::I32, i128::from(value));
        let v = self.md_intern(MdNode::Value(TypeId::I32, v));
        self.add_module_flag(behavior, key, v);
    }

    /// Add `!{!"ident"}` to `!llvm.ident`.
    pub fn add_ident(&mut self, ident: &str) {
        let s = self.md_string(ident.as_bytes());
        let n = self.md_tuple(&[Some(s)]);
        self.add_named_metadata("llvm.ident", &[n]);
    }

    /// The id of metadata kind `name` (`dbg`, `tbaa`, `llvm.loop`, ...).
    pub fn md_kind(&mut self, name: &str) -> MdKindId {
        if let Some(p) = self.md_kinds.iter().position(|k| &**k == name) {
            return MdKindId(p as u32);
        }
        self.md_kinds.push(name.into());
        MdKindId((self.md_kinds.len() - 1) as u32)
    }

    /// A named synchronization scope (`syncscope("agent")`).
    pub fn sync_scope(&mut self, name: &str) -> SyncScope {
        if let Some(p) = self.sync_scopes.iter().position(|k| &**k == name) {
            return SyncScope(p as u8);
        }
        assert!(self.sync_scopes.len() < 255, "too many sync scopes");
        self.sync_scopes.push(name.into());
        SyncScope((self.sync_scopes.len() - 1) as u8)
    }

    /// Set a global variable's address space (interning its pointer type).
    pub fn set_global_addrspace(&mut self, g: GlobalId, addrspace: u32) {
        self.types.ptr(addrspace);
        self.globals[g.index()].addrspace = addrspace;
    }
}

/// Insert or replace a `(kind, node)` attachment, keeping kind order.
pub(crate) fn set_attachment(
    list: &mut Vec<(MdKindId, MetadataId)>,
    kind: MdKindId,
    md: MetadataId,
) {
    match list.binary_search_by_key(&kind, |e| e.0) {
        Ok(i) => list[i].1 = md,
        Err(i) => list.insert(i, (kind, md)),
    }
}

fn mask_words(w: &mut [u64], bits: u32) {
    let rem = bits % 64;
    if rem != 0 {
        let last = w.len() - 1;
        w[last] &= (1u64 << rem) - 1;
    }
}
