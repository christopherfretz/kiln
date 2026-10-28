//! The textual IR printer (`spec/llvm.md` §7).
//!
//! Output follows LLVM 19's `AsmWriter` so that `llvm-as` accepts it and so
//! that, after normalization through `opt -S`, it matches what clang emits:
//! section order, `#N` attribute-group numbering, `!N` metadata numbering
//! and `%N` local numbering are computed the way LLVM's `SlotTracker` and
//! `TypeFinder` compute them. Printing allocates only per-module and
//! per-function side tables, never per instruction.

use std::fmt;
use std::io;

use crate::attr::{Attr, AttrList};
use crate::constant::{ConstData, IntValue, write_signed_words};
use crate::function::{Function, NO_NAME};
use crate::inst::{
    Align, CallConv, Clause, FastMath, Flags, GepFlags, InstKind, Ordering, SyncScope,
};
use crate::metadata::{MdField, MdNode};
use crate::module::{ComdatKind, DllStorage, Linkage, Module, TlsModel, UnnamedAddr, Visibility};
use crate::types::{TypeData, TypeId};
use crate::value::{AttrSetId, BlockId, ComdatId, FuncId, MdKindId, MetadataId, Value, ValueRef};

const NO_SLOT: u32 = u32::MAX;

type Out<'w> = &'w mut dyn fmt::Write;

/// Write `prefix` + `name`, quoting and escaping as LLVM's `PrintLLVMName`.
pub(crate) fn write_llvm_name<W: fmt::Write + ?Sized>(
    w: &mut W,
    prefix: char,
    name: &str,
) -> fmt::Result {
    if prefix != '\0' {
        w.write_char(prefix)?;
    }
    let b = name.as_bytes();
    let needs_quotes = b.first().is_some_and(u8::is_ascii_digit)
        || b.iter().any(|&c| !(c.is_ascii_alphanumeric() || c == b'-' || c == b'.' || c == b'_'));
    if needs_quotes {
        w.write_char('"')?;
        write_escaped(w, b)?;
        w.write_char('"')
    } else {
        w.write_str(name)
    }
}

/// LLVM's `printEscapedString`: printable ASCII except `"` and `\` verbatim,
/// `\` as `\\`, everything else as `\XX`.
pub(crate) fn write_escaped<W: fmt::Write + ?Sized>(w: &mut W, bytes: &[u8]) -> fmt::Result {
    for &c in bytes {
        if c == b'\\' {
            w.write_str("\\\\")?;
        } else if (0x20..0x7f).contains(&c) && c != b'"' {
            w.write_char(c as char)?;
        } else {
            write!(w, "\\{c:02X}")?;
        }
    }
    Ok(())
}

/// LLVM's `printMetadataIdentifier` (named metadata, attachment kinds).
fn write_md_ident(w: Out<'_>, name: &str) -> fmt::Result {
    for (i, &c) in name.as_bytes().iter().enumerate() {
        let ok = if i == 0 { c.is_ascii_alphabetic() } else { c.is_ascii_alphanumeric() }
            || matches!(c, b'-' | b'$' | b'.' | b'_');
        if ok {
            w.write_char(c as char)?;
        } else {
            write!(w, "\\{c:02X}")?;
        }
    }
    Ok(())
}

#[allow(clippy::many_single_char_names)]
fn write_float(w: Out<'_>, m: &Module, ty: TypeId, bits: u128) -> fmt::Result {
    match m.types.data(ty) {
        TypeData::Double => crate::float::write_double(w, f64::from_bits(bits as u64)),
        TypeData::Float => {
            let b = bits as u32;
            let f = f32::from_bits(b);
            if f.is_finite() {
                crate::float::write_double(w, f64::from(f))
            } else {
                // Widen the encoding directly (keeps NaN payloads exactly as
                // APFloat's float-to-double conversion does in AsmWriter).
                let d =
                    (u64::from(b >> 31) << 63) | (0x7ff << 52) | (u64::from(b & 0x7f_ffff) << 29);
                write!(w, "0x{d:X}")
            }
        }
        TypeData::Half => write!(w, "0xH{:04X}", bits as u16),
        TypeData::BFloat => write!(w, "0xR{:04X}", bits as u16),
        TypeData::X86Fp80 => write!(w, "0xK{:04X}{:016X}", (bits >> 64) as u16, bits as u64),
        TypeData::Fp128 => write!(w, "0xL{:016X}{:016X}", bits as u64, (bits >> 64) as u64),
        _ => w.write_str("<bad float>"),
    }
}

fn write_int(w: Out<'_>, m: &Module, ty: TypeId, value: &IntValue) -> fmt::Result {
    let bits = m.types.int_width(ty).unwrap_or(64);
    match value {
        IntValue::Small(v) if bits == 1 => w.write_str(if *v & 1 == 1 { "true" } else { "false" }),
        IntValue::Small(v) => {
            let s = if bits >= 128 {
                *v as i128
            } else {
                let sh = 128 - bits;
                ((*v << sh) as i128) >> sh
            };
            write!(w, "{s}")
        }
        IntValue::Big(words) => write_signed_words(w, words, bits),
    }
}

fn linkage_kw(w: Out<'_>, l: Linkage) -> fmt::Result {
    let k = l.keyword();
    if k.is_empty() { Ok(()) } else { write!(w, "{k} ") }
}

fn write_dso_vis_dll(
    w: Out<'_>,
    linkage: Linkage,
    dso: bool,
    vis: Visibility,
    dll: DllStorage,
) -> fmt::Result {
    let implicit =
        linkage.is_local() || (vis != Visibility::Default && linkage != Linkage::ExternWeak);
    if dso && !implicit {
        w.write_str("dso_local ")?;
    }
    match vis {
        Visibility::Default => {}
        Visibility::Hidden => w.write_str("hidden ")?,
        Visibility::Protected => w.write_str("protected ")?,
    }
    match dll {
        DllStorage::Default => Ok(()),
        DllStorage::Import => w.write_str("dllimport "),
        DllStorage::Export => w.write_str("dllexport "),
    }
}

fn write_tls(w: Out<'_>, t: Option<TlsModel>) -> fmt::Result {
    match t {
        None => Ok(()),
        Some(TlsModel::GeneralDynamic) => w.write_str("thread_local "),
        Some(TlsModel::LocalDynamic) => w.write_str("thread_local(localdynamic) "),
        Some(TlsModel::InitialExec) => w.write_str("thread_local(initialexec) "),
        Some(TlsModel::LocalExec) => w.write_str("thread_local(localexec) "),
    }
}

fn unnamed_addr_kw(u: UnnamedAddr) -> &'static str {
    match u {
        UnnamedAddr::None => "",
        UnnamedAddr::Local => "local_unnamed_addr",
        UnnamedAddr::Global => "unnamed_addr",
    }
}

fn write_cc(w: Out<'_>, cc: CallConv) -> fmt::Result {
    match cc.keyword() {
        None => Ok(()),
        Some("") => write!(w, "cc{}", cc.0),
        Some(k) => w.write_str(k),
    }
}

fn write_fmf(w: Out<'_>, f: FastMath) -> fmt::Result {
    if f == FastMath::FAST {
        return w.write_str(" fast");
    }
    let names = [
        (FastMath::REASSOC, " reassoc"),
        (FastMath::NNAN, " nnan"),
        (FastMath::NINF, " ninf"),
        (FastMath::NSZ, " nsz"),
        (FastMath::ARCP, " arcp"),
        (FastMath::CONTRACT, " contract"),
        (FastMath::AFN, " afn"),
    ];
    for (bit, s) in names {
        if f.0 & bit.0 != 0 {
            w.write_str(s)?;
        }
    }
    Ok(())
}

fn write_flags(w: Out<'_>, f: Flags) -> fmt::Result {
    if f.contains(Flags::NUW) {
        w.write_str(" nuw")?;
    }
    if f.contains(Flags::NSW) {
        w.write_str(" nsw")?;
    }
    if f.contains(Flags::EXACT) {
        w.write_str(" exact")?;
    }
    if f.contains(Flags::DISJOINT) {
        w.write_str(" disjoint")?;
    }
    if f.contains(Flags::NNEG) {
        w.write_str(" nneg")?;
    }
    Ok(())
}

fn write_gep_flags(w: Out<'_>, f: GepFlags, inrange: Option<(i64, i64)>) -> fmt::Result {
    if f.0 & GepFlags::INBOUNDS.0 != 0 {
        w.write_str(" inbounds")?;
    } else if f.0 & GepFlags::NUSW.0 != 0 {
        w.write_str(" nusw")?;
    }
    if f.0 & GepFlags::NUW.0 != 0 {
        w.write_str(" nuw")?;
    }
    if let Some((a, b)) = inrange {
        write!(w, " inrange({a}, {b})")?;
    }
    Ok(())
}

fn write_align(w: Out<'_>, a: Align) -> fmt::Result {
    match a.bytes() {
        Some(n) => write!(w, ", align {n}"),
        None => Ok(()),
    }
}

fn write_shuffle_mask(w: Out<'_>, scalable: bool, mask: &[u32]) -> fmt::Result {
    w.write_str(", <")?;
    if scalable {
        w.write_str("vscale x ")?;
    }
    write!(w, "{} x i32> ", mask.len())?;
    if mask.iter().all(|&e| e == 0) {
        return w.write_str("zeroinitializer");
    }
    if mask.iter().all(|&e| e == crate::builder::SHUFFLE_POISON) {
        return w.write_str("poison");
    }
    w.write_char('<')?;
    for (i, &e) in mask.iter().enumerate() {
        if i > 0 {
            w.write_str(", ")?;
        }
        if e == crate::builder::SHUFFLE_POISON {
            w.write_str("i32 poison")?;
        } else {
            write!(w, "i32 {e}")?;
        }
    }
    w.write_char('>')
}

/// Function-local slot numbers (LLVM's `SlotTracker::processFunction`).
#[derive(Default)]
struct LocalSlots {
    args: Vec<u32>,
    blocks: Vec<u32>,
    insts: Vec<u32>,
}

impl LocalSlots {
    fn compute(&mut self, f: &Function) {
        self.args.clear();
        self.blocks.clear();
        self.insts.clear();
        self.blocks.resize(f.blocks.len(), NO_SLOT);
        self.insts.resize(f.insts.len(), NO_SLOT);
        let mut n = 0u32;
        for name in &f.arg_names {
            if name.is_empty() {
                self.args.push(n);
                n += 1;
            } else {
                self.args.push(NO_SLOT);
            }
        }
        for &bb in &f.layout {
            if f.blocks[bb.index()].name.is_empty() {
                self.blocks[bb.index()] = n;
                n += 1;
            }
            for &i in &f.blocks[bb.index()].insts {
                if f.insts[i.index()].ty != TypeId::VOID && f.inst_names[i.index()] == NO_NAME {
                    self.insts[i.index()] = n;
                    n += 1;
                }
            }
        }
    }
}

struct Printer<'a> {
    m: &'a Module,
    /// Slot of each attribute set used as a function attribute group.
    attr_slot: Vec<u32>,
    attr_order: Vec<AttrSetId>,
    /// Slot of each numbered metadata node.
    md_slot: Vec<u32>,
    md_order: Vec<MetadataId>,
    /// `@N` numbers of unnamed globals, aliases and functions.
    global_slot: Vec<u32>,
    alias_slot: Vec<u32>,
    func_slot: Vec<u32>,
    /// Current function and its slots.
    cur: Option<FuncId>,
    local: LocalSlots,
}

fn is_intrinsic_call(m: &Module, callee: Value) -> bool {
    matches!(callee.get(), ValueRef::Func(f) if m.funcs[f.index()].name.starts_with("llvm."))
}

impl<'a> Printer<'a> {
    fn new(m: &'a Module) -> Printer<'a> {
        let mut p = Printer {
            m,
            attr_slot: vec![NO_SLOT; m.attrs.len()],
            attr_order: Vec::new(),
            md_slot: vec![NO_SLOT; m.md.len()],
            md_order: Vec::new(),
            global_slot: Vec::with_capacity(m.globals.len()),
            alias_slot: Vec::with_capacity(m.aliases.len()),
            func_slot: Vec::with_capacity(m.funcs.len()),
            cur: None,
            local: LocalSlots::default(),
        };
        let mut n = 0u32;
        let mut next = |empty: bool| {
            if empty {
                n += 1;
                n - 1
            } else {
                NO_SLOT
            }
        };
        for g in &m.globals {
            p.global_slot.push(next(g.name.is_empty()));
        }
        for a in &m.aliases {
            p.alias_slot.push(next(a.name.is_empty()));
        }
        for f in &m.funcs {
            p.func_slot.push(next(f.name.is_empty()));
        }
        p.number_attrs();
        p.number_metadata();
        p
    }

    fn add_attr_slot(&mut self, s: AttrSetId) {
        if s != AttrSetId::EMPTY && self.attr_slot[s.index()] == NO_SLOT {
            self.attr_slot[s.index()] = self.attr_order.len() as u32;
            self.attr_order.push(s);
        }
    }

    fn number_attrs(&mut self) {
        let m = self.m;
        for f in &m.funcs {
            self.add_attr_slot(f.attrs.func);
        }
        for f in &m.funcs {
            for &bb in &f.layout {
                for &i in &f.blocks[bb.index()].insts {
                    if let InstKind::Call(c) | InstKind::Invoke(c) = f.insts[i.index()].kind {
                        self.add_attr_slot(f.calls[c as usize].attrs.func);
                    }
                }
            }
        }
    }

    fn add_md_slot(&mut self, root: MetadataId, stack: &mut Vec<MetadataId>) {
        stack.clear();
        stack.push(root);
        while let Some(n) = stack.pop() {
            let node = &self.m.md[n.index()];
            if !node.is_numbered() || self.md_slot[n.index()] != NO_SLOT {
                continue;
            }
            self.md_slot[n.index()] = self.md_order.len() as u32;
            self.md_order.push(n);
            let base = stack.len();
            md_children(node, |c| stack.push(c));
            stack[base..].reverse();
        }
    }

    fn number_metadata(&mut self) {
        let m = self.m;
        let mut stack = Vec::new();
        for g in &m.globals {
            for &(_, md) in &g.md {
                self.add_md_slot(md, &mut stack);
            }
        }
        for (_, ops) in &m.named_md {
            for &md in ops {
                self.add_md_slot(md, &mut stack);
            }
        }
        for f in &m.funcs {
            for &(_, md) in &f.md {
                self.add_md_slot(md, &mut stack);
            }
            for &bb in &f.layout {
                for &i in &f.blocks[bb.index()].insts {
                    if let InstKind::Call(c) = f.insts[i.index()].kind {
                        let call = &f.calls[c as usize];
                        if is_intrinsic_call(m, call.callee) {
                            for &a in f.ops(call.args) {
                                if let ValueRef::Metadata(md) = a.get() {
                                    self.add_md_slot(md, &mut stack);
                                }
                            }
                        }
                    }
                    for &(_, _, md) in f.inst_metadata(i) {
                        self.add_md_slot(md, &mut stack);
                    }
                }
            }
        }
    }

    // ---- operands -------------------------------------------------------

    fn ty(&self, w: Out<'_>, ty: TypeId) -> fmt::Result {
        self.m.types.write(w, ty)
    }

    fn func(&self) -> Option<&'a Function> {
        self.cur.map(|f| &self.m.funcs[f.index()])
    }

    fn value_type(&self, v: Value) -> TypeId {
        match (v.get(), self.func()) {
            (ValueRef::Inst(_) | ValueRef::Arg(_), Some(f)) => {
                f.local_type(v).unwrap_or(TypeId::VOID)
            }
            _ => self.m.const_type(v),
        }
    }

    fn block_ref(w: Out<'_>, f: &Function, slots: &[u32], bb: BlockId) -> fmt::Result {
        let name = &f.blocks[bb.index()].name;
        if name.is_empty() {
            match slots.get(bb.index()) {
                Some(&s) if s != NO_SLOT => write!(w, "%{s}"),
                _ => w.write_str("%<badref>"),
            }
        } else {
            write_llvm_name(w, '%', name)
        }
    }

    /// `WriteAsOperandInternal` without the type.
    #[allow(clippy::many_single_char_names)]
    fn operand(&self, w: Out<'_>, v: Value) -> fmt::Result {
        if v.is_none() {
            return w.write_str("<null operand!>");
        }
        let m = self.m;
        match v.get() {
            ValueRef::Inst(i) => {
                let Some(f) = self.func() else { return w.write_str("<badref>") };
                match f.inst_names.get(i.index()) {
                    Some(&n) if n != NO_NAME => write_llvm_name(w, '%', &f.names[n as usize]),
                    Some(_) => match self.local.insts[i.index()] {
                        NO_SLOT => w.write_str("<badref>"),
                        s => write!(w, "%{s}"),
                    },
                    None => w.write_str("<badref>"),
                }
            }
            ValueRef::Arg(n) => {
                let Some(f) = self.func() else { return w.write_str("<badref>") };
                match f.arg_names.get(n as usize) {
                    Some(name) if !name.is_empty() => write_llvm_name(w, '%', name),
                    Some(_) => write!(w, "%{}", self.local.args[n as usize]),
                    None => w.write_str("<badref>"),
                }
            }
            ValueRef::Block(b) => match self.func() {
                Some(f) => Self::block_ref(w, f, &self.local.blocks, b),
                None => w.write_str("<badref>"),
            },
            ValueRef::Const(c) => self.constant(w, m.consts.get(c)),
            ValueRef::Func(f) => {
                Self::global_name(w, &m.funcs[f.index()].name, self.func_slot[f.index()])
            }
            ValueRef::Global(g) => {
                Self::global_name(w, &m.globals[g.index()].name, self.global_slot[g.index()])
            }
            ValueRef::Alias(a) => {
                Self::global_name(w, &m.aliases[a.index()].name, self.alias_slot[a.index()])
            }
            ValueRef::Asm(a) => {
                let ia = &m.asms[a.index()];
                w.write_str("asm ")?;
                if ia.side_effect {
                    w.write_str("sideeffect ")?;
                }
                if ia.align_stack {
                    w.write_str("alignstack ")?;
                }
                if ia.intel_dialect {
                    w.write_str("inteldialect ")?;
                }
                if ia.can_throw {
                    w.write_str("unwind ")?;
                }
                w.write_char('"')?;
                write_escaped(w, ia.asm.as_bytes())?;
                w.write_str("\", \"")?;
                write_escaped(w, ia.constraints.as_bytes())?;
                w.write_char('"')
            }
            ValueRef::Metadata(md) => self.md_operand(w, md),
        }
    }

    fn global_name(w: Out<'_>, name: &str, slot: u32) -> fmt::Result {
        if name.is_empty() { write!(w, "@{slot}") } else { write_llvm_name(w, '@', name) }
    }

    /// `T v`.
    fn typed(&self, w: Out<'_>, v: Value) -> fmt::Result {
        self.ty(w, self.value_type(v))?;
        w.write_char(' ')?;
        self.operand(w, v)
    }

    #[allow(clippy::many_single_char_names)]
    fn constant(&self, w: Out<'_>, c: &ConstData) -> fmt::Result {
        let m = self.m;
        match c {
            ConstData::Int { ty, value } => write_int(w, m, *ty, value),
            ConstData::Float { ty, bits } => write_float(w, m, *ty, *bits),
            ConstData::Null(_) => w.write_str("null"),
            ConstData::NoneToken => w.write_str("none"),
            ConstData::Zero(_) => w.write_str("zeroinitializer"),
            ConstData::Undef(_) => w.write_str("undef"),
            ConstData::Poison(_) => w.write_str("poison"),
            ConstData::Array { elems, .. } => {
                w.write_char('[')?;
                self.typed_list(w, elems)?;
                w.write_char(']')
            }
            ConstData::Vector { elems, .. } => {
                w.write_char('<')?;
                self.typed_list(w, elems)?;
                w.write_char('>')
            }
            ConstData::Struct { ty, elems } => {
                let packed = m.types.struct_is_packed(*ty);
                if packed {
                    w.write_char('<')?;
                }
                w.write_char('{')?;
                if !elems.is_empty() {
                    w.write_char(' ')?;
                    self.typed_list(w, elems)?;
                    w.write_char(' ')?;
                }
                w.write_char('}')?;
                if packed {
                    w.write_char('>')?;
                }
                Ok(())
            }
            ConstData::Bytes { bytes, .. } => {
                w.write_str("c\"")?;
                write_escaped(w, bytes)?;
                w.write_char('"')
            }
            ConstData::Gep { source, flags, inrange, ops, .. } => {
                w.write_str("getelementptr")?;
                write_gep_flags(w, *flags, *inrange)?;
                w.write_str(" (")?;
                self.ty(w, *source)?;
                w.write_str(", ")?;
                self.typed_list(w, ops)?;
                w.write_char(')')
            }
            ConstData::Cast { op, val, ty } => {
                write!(w, "{} (", op.name())?;
                self.typed(w, *val)?;
                w.write_str(" to ")?;
                self.ty(w, *ty)?;
                w.write_char(')')
            }
            ConstData::Binary { op, flags, lhs, rhs, .. } => {
                w.write_str(op.name())?;
                write_flags(w, *flags)?;
                w.write_str(" (")?;
                self.typed(w, *lhs)?;
                w.write_str(", ")?;
                self.typed(w, *rhs)?;
                w.write_char(')')
            }
            ConstData::BlockAddress { func, block, .. } => {
                w.write_str("blockaddress(")?;
                self.operand(w, Value::func(*func))?;
                w.write_str(", ")?;
                let f = &m.funcs[func.index()];
                if self.cur == Some(*func) {
                    Self::block_ref(w, f, &self.local.blocks, *block)?;
                } else {
                    let mut s = LocalSlots::default();
                    s.compute(f);
                    Self::block_ref(w, f, &s.blocks, *block)?;
                }
                w.write_char(')')
            }
            ConstData::DsoLocalEquivalent(v) => {
                w.write_str("dso_local_equivalent ")?;
                self.operand(w, *v)
            }
            ConstData::NoCfi(v) => {
                w.write_str("no_cfi ")?;
                self.operand(w, *v)
            }
        }
    }

    fn typed_list(&self, w: Out<'_>, vals: &[Value]) -> fmt::Result {
        for (i, &v) in vals.iter().enumerate() {
            if i > 0 {
                w.write_str(", ")?;
            }
            self.typed(w, v)?;
        }
        Ok(())
    }

    // ---- metadata -------------------------------------------------------

    /// A metadata reference: `!N`, `!"s"`, `T v`, or an inline node.
    fn md_operand(&self, w: Out<'_>, md: MetadataId) -> fmt::Result {
        let node = &self.m.md[md.index()];
        match node {
            MdNode::String(s) => {
                w.write_str("!\"")?;
                write_escaped(w, s)?;
                w.write_char('"')
            }
            MdNode::Value(ty, v) => {
                self.ty(w, *ty)?;
                w.write_char(' ')?;
                self.operand(w, *v)
            }
            _ if node.is_numbered() => match self.md_slot[md.index()] {
                NO_SLOT => w.write_str("<badref>"),
                s => write!(w, "!{s}"),
            },
            _ => self.md_body(w, node),
        }
    }

    fn md_body(&self, w: Out<'_>, node: &MdNode) -> fmt::Result {
        if node.is_distinct() {
            w.write_str("distinct ")?;
        }
        match node {
            MdNode::Tuple { ops, .. } => {
                w.write_str("!{")?;
                for (i, op) in ops.iter().enumerate() {
                    if i > 0 {
                        w.write_str(", ")?;
                    }
                    match op {
                        None => w.write_str("null")?,
                        Some(o) => self.md_operand(w, *o)?,
                    }
                }
                w.write_char('}')
            }
            MdNode::Specialized { name, fields, .. } => {
                write!(w, "!{name}(")?;
                for (i, (key, val)) in fields.iter().enumerate() {
                    if i > 0 {
                        w.write_str(", ")?;
                    }
                    if !key.is_empty() {
                        write!(w, "{key}: ")?;
                    }
                    match val {
                        MdField::Int(x) => write!(w, "{x}")?,
                        MdField::UInt(x) => write!(w, "{x}")?,
                        MdField::Bool(b) => write!(w, "{b}")?,
                        MdField::Str(s) => {
                            w.write_char('"')?;
                            write_escaped(w, s.as_bytes())?;
                            w.write_char('"')?;
                        }
                        MdField::Node(None) => w.write_str("null")?,
                        MdField::Node(Some(n)) => self.md_operand(w, *n)?,
                        MdField::Raw(r) => w.write_str(r)?,
                    }
                }
                w.write_char(')')
            }
            MdNode::String(_) | MdNode::Value(..) => Ok(()),
        }
    }

    fn attachments(
        &self,
        w: Out<'_>,
        list: impl Iterator<Item = (MdKindId, MetadataId)>,
        sep: &str,
    ) -> fmt::Result {
        for (kind, md) in list {
            w.write_str(sep)?;
            w.write_char('!')?;
            write_md_ident(w, &self.m.md_kinds[kind.index()])?;
            w.write_char(' ')?;
            self.md_operand(w, md)?;
        }
        Ok(())
    }

    // ---- attributes -----------------------------------------------------

    /// Inline attribute set (`noundef align 8`).
    fn attr_set(&self, w: Out<'_>, s: AttrSetId) -> fmt::Result {
        for (i, a) in self.m.attrs.get(s).iter().enumerate() {
            if i > 0 {
                w.write_char(' ')?;
            }
            a.write(w, &self.m.types)?;
        }
        Ok(())
    }

    fn attr_group_ref(&self, w: Out<'_>, s: AttrSetId) -> fmt::Result {
        if s != AttrSetId::EMPTY {
            write!(w, " #{}", self.attr_slot[s.index()])?;
        }
        Ok(())
    }

    // ---- module ---------------------------------------------------------

    fn module(&mut self, w: Out<'_>) -> fmt::Result {
        let m = self.m;
        if !m.module_id.is_empty() && !m.module_id.contains('\n') {
            writeln!(w, "; ModuleID = '{}'", m.module_id)?;
        }
        if !m.source_filename.is_empty() {
            w.write_str("source_filename = \"")?;
            write_escaped(w, m.source_filename.as_bytes())?;
            w.write_str("\"\n")?;
        }
        if !m.data_layout.is_empty() {
            writeln!(w, "target datalayout = \"{}\"", m.data_layout)?;
        }
        if !m.target_triple.is_empty() {
            writeln!(w, "target triple = \"{}\"", m.target_triple)?;
        }
        if !m.module_asm.is_empty() {
            w.write_char('\n')?;
            for chunk in &m.module_asm {
                for line in chunk.split('\n') {
                    w.write_str("module asm \"")?;
                    write_escaped(w, line.as_bytes())?;
                    w.write_str("\"\n")?;
                }
            }
        }
        self.type_identities(w)?;
        self.comdats(w)?;
        if !m.globals.is_empty() {
            w.write_char('\n')?;
        }
        for i in 0..m.globals.len() {
            self.global(w, i)?;
            w.write_char('\n')?;
        }
        if !m.aliases.is_empty() {
            w.write_char('\n')?;
        }
        for i in 0..m.aliases.len() {
            self.alias(w, i)?;
        }
        for i in 0..m.funcs.len() {
            w.write_char('\n')?;
            self.function(w, FuncId(i as u32))?;
        }
        if !self.attr_order.is_empty() {
            w.write_char('\n')?;
            for (n, &s) in self.attr_order.iter().enumerate() {
                write!(w, "attributes #{n} = {{ ")?;
                for (i, a) in m.attrs.get(s).iter().enumerate() {
                    if i > 0 {
                        w.write_char(' ')?;
                    }
                    match a {
                        Attr::Align(n) => write!(w, "align={n}")?,
                        Attr::AlignStack(n) => write!(w, "alignstack={n}")?,
                        _ => a.write(w, &m.types)?,
                    }
                }
                w.write_str(" }\n")?;
            }
        }
        if !m.named_md.is_empty() {
            w.write_char('\n')?;
        }
        for (name, ops) in &m.named_md {
            w.write_char('!')?;
            write_md_ident(w, name)?;
            w.write_str(" = !{")?;
            for (i, &op) in ops.iter().enumerate() {
                if i > 0 {
                    w.write_str(", ")?;
                }
                self.md_operand(w, op)?;
            }
            w.write_str("}\n")?;
        }
        if !self.md_order.is_empty() {
            w.write_char('\n')?;
            for (n, &md) in self.md_order.iter().enumerate() {
                write!(w, "!{n} = ")?;
                self.md_body(w, &m.md[md.index()])?;
                w.write_char('\n')?;
            }
        }
        Ok(())
    }

    fn type_identities(&self, w: Out<'_>) -> fmt::Result {
        let t = &self.m.types;
        let unnamed: Vec<TypeId> = t
            .named_structs()
            .iter()
            .copied()
            .filter(|&s| t.struct_name(s).is_some_and(str::is_empty))
            .collect();
        let named = TypeFinder::run(self.m);
        if unnamed.is_empty() && named.is_empty() {
            return Ok(());
        }
        w.write_char('\n')?;
        for ty in unnamed.into_iter().chain(named) {
            t.write_definition(w, ty)?;
            w.write_char('\n')?;
        }
        Ok(())
    }

    fn comdats(&self, w: Out<'_>) -> fmt::Result {
        let m = self.m;
        let mut seen = vec![false; m.comdats.len()];
        let mut order: Vec<ComdatId> = Vec::new();
        let used = m
            .funcs
            .iter()
            .filter_map(|f| f.comdat)
            .chain(m.globals.iter().filter_map(|g| g.comdat));
        for c in used {
            if !seen[c.index()] {
                seen[c.index()] = true;
                order.push(c);
            }
        }
        if !order.is_empty() {
            w.write_char('\n')?;
        }
        for (i, &c) in order.iter().enumerate() {
            let cd = &m.comdats[c.index()];
            write_llvm_name(w, '$', &cd.name)?;
            w.write_str(match cd.kind {
                ComdatKind::Any => " = comdat any\n",
                ComdatKind::ExactMatch => " = comdat exactmatch\n",
                ComdatKind::Largest => " = comdat largest\n",
                ComdatKind::NoDeduplicate => " = comdat nodeduplicate\n",
                ComdatKind::SameSize => " = comdat samesize\n",
            })?;
            if i + 1 != order.len() {
                w.write_char('\n')?;
            }
        }
        Ok(())
    }

    fn comdat_ref(
        &self,
        w: Out<'_>,
        c: Option<ComdatId>,
        obj_name: &str,
        is_var: bool,
    ) -> fmt::Result {
        let Some(c) = c else { return Ok(()) };
        if is_var {
            w.write_char(',')?;
        }
        w.write_str(" comdat")?;
        let name = &self.m.comdats[c.index()].name;
        if &**name != obj_name {
            w.write_char('(')?;
            write_llvm_name(w, '$', name)?;
            w.write_char(')')?;
        }
        Ok(())
    }

    fn global(&self, w: Out<'_>, i: usize) -> fmt::Result {
        let g = &self.m.globals[i];
        Self::global_name(w, &g.name, self.global_slot[i])?;
        w.write_str(" = ")?;
        if g.init.is_none() && g.linkage == Linkage::External {
            w.write_str("external ")?;
        }
        linkage_kw(w, g.linkage)?;
        write_dso_vis_dll(w, g.linkage, g.dso_local, g.visibility, g.dll_storage)?;
        write_tls(w, g.thread_local)?;
        let ua = unnamed_addr_kw(g.unnamed_addr);
        if !ua.is_empty() {
            write!(w, "{ua} ")?;
        }
        if g.addrspace != 0 {
            write!(w, "addrspace({}) ", g.addrspace)?;
        }
        if g.externally_initialized {
            w.write_str("externally_initialized ")?;
        }
        w.write_str(if g.is_constant { "constant " } else { "global " })?;
        self.ty(w, g.value_type)?;
        if let Some(init) = g.init {
            w.write_char(' ')?;
            self.operand(w, init)?;
        }
        if let Some(s) = &g.section {
            w.write_str(", section \"")?;
            write_escaped(w, s.as_bytes())?;
            w.write_char('"')?;
        }
        self.comdat_ref(w, g.comdat, &g.name, true)?;
        if let Some(a) = g.align {
            write!(w, ", align {a}")?;
        }
        self.attachments(w, g.md.iter().copied(), ", ")
    }

    fn alias(&self, w: Out<'_>, i: usize) -> fmt::Result {
        let a = &self.m.aliases[i];
        Self::global_name(w, &a.name, self.alias_slot[i])?;
        w.write_str(" = ")?;
        linkage_kw(w, a.linkage)?;
        write_dso_vis_dll(w, a.linkage, a.dso_local, a.visibility, a.dll_storage)?;
        write_tls(w, a.thread_local)?;
        let ua = unnamed_addr_kw(a.unnamed_addr);
        if !ua.is_empty() {
            write!(w, "{ua} ")?;
        }
        w.write_str("alias ")?;
        self.ty(w, a.value_type)?;
        w.write_str(", ")?;
        let is_expr = matches!(
            self.m.const_of(a.aliasee),
            Some(ConstData::Gep { .. } | ConstData::Cast { .. } | ConstData::Binary { .. })
        );
        if is_expr {
            self.operand(w, a.aliasee)?;
        } else {
            self.typed(w, a.aliasee)?;
        }
        w.write_char('\n')
    }

    fn function(&mut self, w: Out<'_>, fid: FuncId) -> fmt::Result {
        let f = &self.m.funcs[fid.index()];
        self.cur = Some(fid);
        self.local.compute(f);
        let decl = f.is_declaration();
        if decl {
            w.write_str("declare")?;
            self.attachments(w, f.md.iter().copied(), " ")?;
            w.write_char(' ')?;
        } else {
            w.write_str("define ")?;
        }
        linkage_kw(w, f.linkage)?;
        write_dso_vis_dll(w, f.linkage, f.dso_local, f.visibility, f.dll_storage)?;
        if f.cc != CallConv::C {
            write_cc(w, f.cc)?;
            w.write_char(' ')?;
        }
        if f.attrs.ret != AttrSetId::EMPTY {
            self.attr_set(w, f.attrs.ret)?;
            w.write_char(' ')?;
        }
        self.ty(w, f.ret)?;
        w.write_char(' ')?;
        Self::global_name(w, &f.name, self.func_slot[fid.index()])?;
        w.write_char('(')?;
        for (i, &pt) in f.params.iter().enumerate() {
            if i > 0 {
                w.write_str(", ")?;
            }
            self.ty(w, pt)?;
            let pa = f.attrs.param(i);
            if pa != AttrSetId::EMPTY {
                w.write_char(' ')?;
                self.attr_set(w, pa)?;
            }
            if !decl {
                w.write_char(' ')?;
                self.operand(w, Value::arg(i as u32))?;
            }
        }
        if f.varargs {
            w.write_str(if f.params.is_empty() { "..." } else { ", ..." })?;
        }
        w.write_char(')')?;
        let ua = unnamed_addr_kw(f.unnamed_addr);
        if !ua.is_empty() {
            write!(w, " {ua}")?;
        }
        if f.addrspace != 0 {
            write!(w, " addrspace({})", f.addrspace)?;
        }
        self.attr_group_ref(w, f.attrs.func)?;
        if let Some(s) = &f.section {
            w.write_str(" section \"")?;
            write_escaped(w, s.as_bytes())?;
            w.write_char('"')?;
        }
        self.comdat_ref(w, f.comdat, &f.name, false)?;
        if let Some(a) = f.align {
            write!(w, " align {a}")?;
        }
        if let Some(gc) = &f.gc {
            w.write_str(" gc \"")?;
            write_escaped(w, gc.as_bytes())?;
            w.write_char('"')?;
        }
        if let Some(p) = f.personality {
            w.write_str(" personality ")?;
            self.typed(w, p)?;
        }
        if decl {
            w.write_char('\n')?;
        } else {
            self.attachments(w, f.md.iter().copied(), " ")?;
            w.write_str(" {")?;
            for (bi, &bb) in f.layout.iter().enumerate() {
                let name = &f.blocks[bb.index()].name;
                if !name.is_empty() {
                    w.write_char('\n')?;
                    write_llvm_name(w, '\0', name)?;
                    w.write_char(':')?;
                } else if bi != 0 {
                    write!(w, "\n{}:", self.local.blocks[bb.index()])?;
                }
                w.write_char('\n')?;
                for &i in &f.blocks[bb.index()].insts {
                    self.instruction(w, f, i)?;
                    w.write_char('\n')?;
                }
            }
            w.write_str("}\n")?;
        }
        self.cur = None;
        Ok(())
    }

    #[allow(clippy::too_many_lines)]
    fn instruction(&self, w: Out<'_>, f: &Function, id: crate::value::InstId) -> fmt::Result {
        let inst = &f.insts[id.index()];
        w.write_str("  ")?;
        if inst.ty != TypeId::VOID {
            self.operand(w, Value::inst(id))?;
            w.write_str(" = ")?;
        }
        if let InstKind::Call(c) = inst.kind {
            match f.calls[c as usize].tail {
                crate::inst::TailKind::None => {}
                crate::inst::TailKind::Tail => w.write_str("tail ")?,
                crate::inst::TailKind::MustTail => w.write_str("musttail ")?,
                crate::inst::TailKind::NoTail => w.write_str("notail ")?,
            }
        }
        match inst.kind {
            InstKind::Binary { op, flags, fmf, lhs, rhs } => {
                w.write_str(op.name())?;
                write_fmf(w, fmf)?;
                write_flags(w, flags)?;
                self.same_typed(w, &[lhs, rhs])?;
            }
            InstKind::FNeg { fmf, val } => {
                w.write_str("fneg")?;
                write_fmf(w, fmf)?;
                self.same_typed(w, &[val])?;
            }
            InstKind::ICmp { pred, lhs, rhs } => {
                write!(w, "icmp {}", pred.name())?;
                self.same_typed(w, &[lhs, rhs])?;
            }
            InstKind::FCmp { pred, fmf, lhs, rhs } => {
                w.write_str("fcmp")?;
                write_fmf(w, fmf)?;
                write!(w, " {}", pred.name())?;
                self.same_typed(w, &[lhs, rhs])?;
            }
            InstKind::Cast { op, flags, val } => {
                w.write_str(op.name())?;
                write_flags(w, flags)?;
                w.write_char(' ')?;
                self.typed(w, val)?;
                w.write_str(" to ")?;
                self.ty(w, inst.ty)?;
            }
            InstKind::Alloca { allocated, count, align, inalloca } => {
                w.write_str("alloca ")?;
                if inalloca {
                    w.write_str("inalloca ")?;
                }
                self.ty(w, allocated)?;
                if !count.is_none() {
                    w.write_str(", ")?;
                    self.typed(w, count)?;
                }
                write_align(w, align)?;
            }
            InstKind::Load { ptr, align, volatile, ordering, scope } => {
                w.write_str("load")?;
                if ordering != Ordering::NotAtomic {
                    w.write_str(" atomic")?;
                }
                if volatile {
                    w.write_str(" volatile")?;
                }
                w.write_char(' ')?;
                self.ty(w, inst.ty)?;
                w.write_str(", ")?;
                self.typed(w, ptr)?;
                self.atomic(w, ordering, scope)?;
                write_align(w, align)?;
            }
            InstKind::Store { val, ptr, align, volatile, ordering, scope } => {
                w.write_str("store")?;
                if ordering != Ordering::NotAtomic {
                    w.write_str(" atomic")?;
                }
                if volatile {
                    w.write_str(" volatile")?;
                }
                w.write_char(' ')?;
                self.typed(w, val)?;
                w.write_str(", ")?;
                self.typed(w, ptr)?;
                self.atomic(w, ordering, scope)?;
                write_align(w, align)?;
            }
            InstKind::Fence { ordering, scope } => {
                w.write_str("fence")?;
                self.atomic(w, ordering, scope)?;
            }
            InstKind::CmpXchg { ptr, cmp, new, align, success, failure, weak, volatile, scope } => {
                w.write_str("cmpxchg")?;
                if weak {
                    w.write_str(" weak")?;
                }
                if volatile {
                    w.write_str(" volatile")?;
                }
                w.write_char(' ')?;
                self.typed(w, ptr)?;
                w.write_str(", ")?;
                self.typed(w, cmp)?;
                w.write_str(", ")?;
                self.typed(w, new)?;
                self.sync_scope(w, scope)?;
                write!(w, " {} {}", success.name(), failure.name())?;
                write_align(w, align)?;
            }
            InstKind::AtomicRmw { op, ptr, val, align, ordering, volatile, scope } => {
                w.write_str("atomicrmw")?;
                if volatile {
                    w.write_str(" volatile")?;
                }
                write!(w, " {} ", op.name())?;
                self.typed(w, ptr)?;
                w.write_str(", ")?;
                self.typed(w, val)?;
                self.atomic(w, ordering, scope)?;
                write_align(w, align)?;
            }
            InstKind::Gep { source, flags, ops } => {
                w.write_str("getelementptr")?;
                write_gep_flags(w, flags, None)?;
                w.write_char(' ')?;
                self.ty(w, source)?;
                w.write_char(',')?;
                self.same_typed(w, f.ops(ops))?;
            }
            InstKind::ExtractValue { agg, idx } => {
                w.write_str("extractvalue ")?;
                self.typed(w, agg)?;
                for i in f.idx(idx) {
                    write!(w, ", {i}")?;
                }
            }
            InstKind::InsertValue { agg, val, idx } => {
                w.write_str("insertvalue ")?;
                self.typed(w, agg)?;
                w.write_str(", ")?;
                self.typed(w, val)?;
                for i in f.idx(idx) {
                    write!(w, ", {i}")?;
                }
            }
            InstKind::ExtractElement { vec, idx } => {
                w.write_str("extractelement")?;
                self.same_typed(w, &[vec, idx])?;
            }
            InstKind::InsertElement { vec, elt, idx } => {
                w.write_str("insertelement")?;
                self.same_typed(w, &[vec, elt, idx])?;
            }
            InstKind::ShuffleVector { a, b, mask } => {
                w.write_str("shufflevector ")?;
                self.typed(w, a)?;
                w.write_str(", ")?;
                self.typed(w, b)?;
                let scalable = self.m.types.vector_info(inst.ty).is_some_and(|v| v.2);
                write_shuffle_mask(w, scalable, f.idx(mask))?;
            }
            InstKind::Phi { fmf, incoming } => {
                w.write_str("phi")?;
                write_fmf(w, fmf)?;
                w.write_char(' ')?;
                self.ty(w, inst.ty)?;
                w.write_char(' ')?;
                for (i, pair) in f.ops(incoming).chunks(2).enumerate() {
                    if i > 0 {
                        w.write_str(", ")?;
                    }
                    w.write_str("[ ")?;
                    self.operand(w, pair[0])?;
                    w.write_str(", ")?;
                    self.operand(w, pair[1])?;
                    w.write_str(" ]")?;
                }
            }
            InstKind::Select { fmf, cond, t, f: fv } => {
                w.write_str("select")?;
                write_fmf(w, fmf)?;
                w.write_char(' ')?;
                self.typed(w, cond)?;
                w.write_str(", ")?;
                self.typed(w, t)?;
                w.write_str(", ")?;
                self.typed(w, fv)?;
            }
            InstKind::Call(c) | InstKind::Invoke(c) => {
                let call = &f.calls[c as usize];
                let is_invoke = matches!(inst.kind, InstKind::Invoke(_));
                w.write_str(if is_invoke { "invoke" } else { "call" })?;
                write_fmf(w, call.fmf)?;
                if call.cc != CallConv::C {
                    w.write_char(' ')?;
                    write_cc(w, call.cc)?;
                }
                if call.attrs.ret != AttrSetId::EMPTY {
                    w.write_char(' ')?;
                    self.attr_set(w, call.attrs.ret)?;
                }
                if let Some(a) = self.m.types.ptr_addrspace(self.value_type(call.callee))
                    && a != 0
                {
                    write!(w, " addrspace({a})")?;
                }
                w.write_char(' ')?;
                let (ret, _, varargs) =
                    self.m.types.function_info(call.fn_ty).unwrap_or((TypeId::VOID, &[], false));
                self.ty(w, if varargs { call.fn_ty } else { ret })?;
                w.write_char(' ')?;
                self.operand(w, call.callee)?;
                w.write_char('(')?;
                self.call_args(w, f.ops(call.args), &call.attrs)?;
                if call.tail == crate::inst::TailKind::MustTail && f.varargs {
                    w.write_str(if call.args.len > 0 { ", ..." } else { "..." })?;
                }
                w.write_char(')')?;
                self.attr_group_ref(w, call.attrs.func)?;
                if !call.bundles.is_empty() {
                    w.write_str(" [ ")?;
                    for (i, b) in call.bundles.iter().enumerate() {
                        if i > 0 {
                            w.write_str(", ")?;
                        }
                        w.write_char('"')?;
                        write_escaped(w, b.tag.as_bytes())?;
                        w.write_str("\"(")?;
                        self.typed_list(w, &b.args)?;
                        w.write_char(')')?;
                    }
                    w.write_str(" ]")?;
                }
                if let (Some(n), Some(u)) = (call.normal, call.unwind) {
                    w.write_str("\n          to label ")?;
                    self.operand(w, Value::block(n))?;
                    w.write_str(" unwind label ")?;
                    self.operand(w, Value::block(u))?;
                }
            }
            InstKind::LandingPad(p) => {
                let pad = &f.pads[p as usize];
                w.write_str("landingpad ")?;
                self.ty(w, inst.ty)?;
                if pad.cleanup || !pad.clauses.is_empty() {
                    w.write_char('\n')?;
                }
                if pad.cleanup {
                    w.write_str("          cleanup")?;
                }
                for (i, c) in pad.clauses.iter().enumerate() {
                    if i != 0 || pad.cleanup {
                        w.write_char('\n')?;
                    }
                    match *c {
                        Clause::Catch(v) => {
                            w.write_str("          catch ")?;
                            self.typed(w, v)?;
                        }
                        Clause::Filter(v) => {
                            w.write_str("          filter ")?;
                            self.typed(w, v)?;
                        }
                    }
                }
            }
            InstKind::Resume { val } => {
                w.write_str("resume ")?;
                self.typed(w, val)?;
            }
            InstKind::CatchSwitch { parent, handlers, unwind } => {
                w.write_str("catchswitch within ")?;
                self.pad_parent(w, parent)?;
                w.write_str(" [")?;
                for (i, &h) in f.ops(handlers).iter().enumerate() {
                    if i > 0 {
                        w.write_str(", ")?;
                    }
                    w.write_str("label ")?;
                    self.operand(w, h)?;
                }
                w.write_str("] unwind ")?;
                self.unwind_dest(w, unwind)?;
            }
            InstKind::CatchPad { parent, args } | InstKind::CleanupPad { parent, args } => {
                w.write_str(if matches!(inst.kind, InstKind::CatchPad { .. }) {
                    "catchpad within "
                } else {
                    "cleanuppad within "
                })?;
                self.pad_parent(w, parent)?;
                w.write_str(" [")?;
                self.typed_list(w, f.ops(args))?;
                w.write_char(']')?;
            }
            InstKind::CatchRet { pad, dest } => {
                w.write_str("catchret from ")?;
                self.operand(w, pad)?;
                w.write_str(" to label ")?;
                self.operand(w, Value::block(dest))?;
            }
            InstKind::CleanupRet { pad, unwind } => {
                w.write_str("cleanupret from ")?;
                self.operand(w, pad)?;
                w.write_str(" unwind ")?;
                self.unwind_dest(w, unwind)?;
            }
            InstKind::Br { dest } => {
                w.write_str("br label ")?;
                self.operand(w, Value::block(dest))?;
            }
            InstKind::CondBr { cond, t, f: fb } => {
                w.write_str("br ")?;
                self.typed(w, cond)?;
                w.write_str(", label ")?;
                self.operand(w, Value::block(t))?;
                w.write_str(", label ")?;
                self.operand(w, Value::block(fb))?;
            }
            InstKind::Switch { cond, default, cases } => {
                w.write_str("switch ")?;
                self.typed(w, cond)?;
                w.write_str(", label ")?;
                self.operand(w, Value::block(default))?;
                w.write_str(" [")?;
                for pair in f.ops(cases).chunks(2) {
                    w.write_str("\n    ")?;
                    self.typed(w, pair[0])?;
                    w.write_str(", label ")?;
                    self.operand(w, pair[1])?;
                }
                w.write_str("\n  ]")?;
            }
            InstKind::IndirectBr { addr, dests } => {
                w.write_str("indirectbr ")?;
                self.typed(w, addr)?;
                w.write_str(", [")?;
                for (i, &d) in f.ops(dests).iter().enumerate() {
                    if i > 0 {
                        w.write_str(", ")?;
                    }
                    w.write_str("label ")?;
                    self.operand(w, d)?;
                }
                w.write_char(']')?;
            }
            InstKind::Ret { val } => {
                if val.is_none() {
                    w.write_str("ret void")?;
                } else {
                    w.write_str("ret ")?;
                    self.typed(w, val)?;
                }
            }
            InstKind::Unreachable => w.write_str("unreachable")?,
            InstKind::Freeze { val } => {
                w.write_str("freeze ")?;
                self.typed(w, val)?;
            }
            InstKind::VaArg { list } => {
                w.write_str("va_arg ")?;
                self.typed(w, list)?;
                w.write_str(", ")?;
                self.ty(w, inst.ty)?;
            }
        }
        self.attachments(w, f.inst_metadata(id).iter().map(|e| (e.1, e.2)), ", ")
    }

    /// The generic operand form: one leading type when all operand types
    /// agree, else a type before every operand.
    fn same_typed(&self, w: Out<'_>, ops: &[Value]) -> fmt::Result {
        let Some(&first) = ops.first() else { return Ok(()) };
        let t0 = self.value_type(first);
        let all = ops[1..].iter().any(|&o| self.value_type(o) != t0);
        if !all {
            w.write_char(' ')?;
            self.ty(w, t0)?;
        }
        w.write_char(' ')?;
        for (i, &o) in ops.iter().enumerate() {
            if i > 0 {
                w.write_str(", ")?;
            }
            if all {
                self.typed(w, o)?;
            } else {
                self.operand(w, o)?;
            }
        }
        Ok(())
    }

    fn call_args(&self, w: Out<'_>, args: &[Value], attrs: &AttrList) -> fmt::Result {
        for (i, &a) in args.iter().enumerate() {
            if i > 0 {
                w.write_str(", ")?;
            }
            self.ty(w, self.value_type(a))?;
            let pa = attrs.param(i);
            if pa != AttrSetId::EMPTY {
                w.write_char(' ')?;
                self.attr_set(w, pa)?;
            }
            w.write_char(' ')?;
            self.operand(w, a)?;
        }
        Ok(())
    }

    fn pad_parent(&self, w: Out<'_>, parent: Value) -> fmt::Result {
        if parent.is_none() || matches!(self.m.const_of(parent), Some(ConstData::NoneToken)) {
            w.write_str("none")
        } else {
            self.operand(w, parent)
        }
    }

    fn unwind_dest(&self, w: Out<'_>, unwind: Value) -> fmt::Result {
        if unwind.is_none() {
            w.write_str("to caller")
        } else {
            w.write_str("label ")?;
            self.operand(w, unwind)
        }
    }

    fn sync_scope(&self, w: Out<'_>, s: SyncScope) -> fmt::Result {
        if s != SyncScope::SYSTEM {
            w.write_str(" syncscope(\"")?;
            write_escaped(w, self.m.sync_scopes[s.0 as usize].as_bytes())?;
            w.write_str("\")")?;
        }
        Ok(())
    }

    fn atomic(&self, w: Out<'_>, o: Ordering, s: SyncScope) -> fmt::Result {
        if o == Ordering::NotAtomic {
            return Ok(());
        }
        self.sync_scope(w, s)?;
        write!(w, " {}", o.name())
    }
}

/// Children of a metadata node that are themselves nodes, in operand order.
fn md_children(node: &MdNode, mut push: impl FnMut(MetadataId)) {
    match node {
        MdNode::Tuple { ops, .. } => ops.iter().flatten().for_each(|&o| push(o)),
        MdNode::Specialized { fields, .. } => {
            for (_, f) in fields {
                if let MdField::Node(Some(n)) = f {
                    push(*n);
                }
            }
        }
        MdNode::String(_) | MdNode::Value(..) => {}
    }
}

/// LLVM's `TypeFinder`: identified structs in the order the printer
/// discovers them (globals, aliases, functions, named metadata).
struct TypeFinder<'a> {
    m: &'a Module,
    seen_ty: Vec<bool>,
    seen_const: Vec<bool>,
    seen_md: Vec<bool>,
    out: Vec<TypeId>,
    work: Vec<TypeId>,
}

impl<'a> TypeFinder<'a> {
    fn run(m: &'a Module) -> Vec<TypeId> {
        let mut t = TypeFinder {
            m,
            seen_ty: vec![false; m.types.len()],
            seen_const: vec![false; m.consts.len()],
            seen_md: vec![false; m.md.len()],
            out: Vec::new(),
            work: Vec::new(),
        };
        for g in &m.globals {
            t.ty(g.value_type);
            if let Some(i) = g.init {
                t.value(i);
            }
        }
        for a in &m.aliases {
            t.ty(a.value_type);
            t.value(a.aliasee);
        }
        for f in &m.funcs {
            t.ty(f.ty);
            t.attrs(&f.attrs);
            if let Some(p) = f.personality {
                t.value(p);
            }
            for &bb in &f.layout {
                for &i in &f.blocks[bb.index()].insts {
                    t.inst(f, i);
                }
            }
        }
        for (_, ops) in &m.named_md {
            for &n in ops {
                t.md(n);
            }
        }
        t.out
    }

    #[allow(clippy::too_many_lines)]
    fn inst(&mut self, f: &Function, id: crate::value::InstId) {
        let inst = &f.insts[id.index()];
        self.ty(inst.ty);
        let mut vals = |v: Value| {
            if !v.is_none() && !matches!(v.get(), ValueRef::Inst(_)) {
                self.value(v);
            }
        };
        match inst.kind {
            InstKind::Binary { lhs, rhs, .. }
            | InstKind::ICmp { lhs, rhs, .. }
            | InstKind::FCmp { lhs, rhs, .. } => {
                vals(lhs);
                vals(rhs);
            }
            InstKind::FNeg { val, .. }
            | InstKind::Cast { val, .. }
            | InstKind::Resume { val }
            | InstKind::Ret { val }
            | InstKind::Freeze { val } => vals(val),
            InstKind::Alloca { count, .. } => vals(count),
            InstKind::Load { ptr, .. } => vals(ptr),
            InstKind::Store { val, ptr, .. } => {
                vals(val);
                vals(ptr);
            }
            InstKind::Fence { .. } | InstKind::Unreachable | InstKind::Br { .. } => {}
            InstKind::CmpXchg { ptr, cmp, new, .. } => {
                vals(ptr);
                vals(cmp);
                vals(new);
            }
            InstKind::AtomicRmw { ptr, val, .. } => {
                vals(ptr);
                vals(val);
            }
            InstKind::Gep { ops, .. } => f.ops(ops).iter().for_each(|&v| vals(v)),
            InstKind::ExtractValue { agg, .. } => vals(agg),
            InstKind::InsertValue { agg, val, .. } => {
                vals(agg);
                vals(val);
            }
            InstKind::ExtractElement { vec, idx } => {
                vals(vec);
                vals(idx);
            }
            InstKind::InsertElement { vec, elt, idx } => {
                vals(vec);
                vals(elt);
                vals(idx);
            }
            InstKind::ShuffleVector { a, b, .. } => {
                vals(a);
                vals(b);
            }
            InstKind::Phi { incoming, .. } => {
                f.ops(incoming).iter().step_by(2).for_each(|&v| vals(v));
            }
            InstKind::Select { cond, t, f: fv, .. } => {
                vals(cond);
                vals(t);
                vals(fv);
            }
            InstKind::Call(c) | InstKind::Invoke(c) => {
                let call = &f.calls[c as usize];
                // Operand order: args, bundle operands, (invoke dests), callee.
                f.ops(call.args).iter().for_each(|&v| vals(v));
                for b in &call.bundles {
                    b.args.iter().for_each(|&v| vals(v));
                }
                vals(call.callee);
            }
            InstKind::LandingPad(p) => {
                for c in &f.pads[p as usize].clauses {
                    match *c {
                        Clause::Catch(v) | Clause::Filter(v) => vals(v),
                    }
                }
            }
            InstKind::CatchSwitch { parent, .. } => vals(parent),
            InstKind::CatchPad { parent, args } | InstKind::CleanupPad { parent, args } => {
                f.ops(args).iter().for_each(|&v| vals(v));
                vals(parent);
            }
            InstKind::CatchRet { pad, .. } | InstKind::CleanupRet { pad, .. } => vals(pad),
            InstKind::CondBr { cond, .. } => vals(cond),
            InstKind::Switch { cond, cases, .. } => {
                vals(cond);
                f.ops(cases).iter().step_by(2).for_each(|&v| vals(v));
            }
            InstKind::IndirectBr { addr, .. } => vals(addr),
            InstKind::VaArg { list } => vals(list),
        }
        match inst.kind {
            InstKind::Gep { source, .. } => self.ty(source),
            InstKind::Alloca { allocated, .. } => self.ty(allocated),
            InstKind::Call(c) | InstKind::Invoke(c) => self.attrs(&f.calls[c as usize].attrs),
            _ => {}
        }
        let dbg = MdKindId(0);
        for &(_, kind, md) in f.inst_metadata(id) {
            if kind != dbg {
                self.md(md);
            }
        }
    }

    fn attrs(&mut self, l: &AttrList) {
        let m = self.m;
        let sets = [l.func, l.ret].into_iter().chain(l.params.iter().copied());
        for s in sets {
            for a in m.attrs.get(s) {
                match a {
                    Attr::ByRef(t)
                    | Attr::ByVal(t)
                    | Attr::ElementType(t)
                    | Attr::InAlloca(t)
                    | Attr::Preallocated(t)
                    | Attr::SRet(t) => self.ty(*t),
                    _ => {}
                }
            }
        }
    }

    fn ty(&mut self, ty: TypeId) {
        if std::mem::replace(&mut self.seen_ty[ty.index()], true) {
            return;
        }
        let types = &self.m.types;
        self.work.push(ty);
        while let Some(t) = self.work.pop() {
            let data = types.data(t);
            if let TypeData::Named(_) = data
                && types.struct_name(t).is_some_and(|n| !n.is_empty())
            {
                self.out.push(t);
            }
            let subs: &[TypeId] = match data {
                TypeData::Vector { elem, .. } | TypeData::Array { elem, .. } => {
                    std::slice::from_ref(elem)
                }
                TypeData::Struct { fields, .. } => fields,
                TypeData::Named(_) => types.struct_fields(t).unwrap_or(&[]),
                TypeData::Function { ret, params, .. } => {
                    for &p in params.iter().rev() {
                        if !std::mem::replace(&mut self.seen_ty[p.index()], true) {
                            self.work.push(p);
                        }
                    }
                    std::slice::from_ref(ret)
                }
                _ => &[],
            };
            for &s in subs.iter().rev() {
                if !std::mem::replace(&mut self.seen_ty[s.index()], true) {
                    self.work.push(s);
                }
            }
        }
    }

    fn value(&mut self, v: Value) {
        match v.get() {
            ValueRef::Metadata(md) => match &self.m.md[md.index()] {
                MdNode::Value(_, inner) => self.value(*inner),
                MdNode::String(_) => {}
                _ => self.md(md),
            },
            ValueRef::Const(c) => {
                if std::mem::replace(&mut self.seen_const[c.index()], true) {
                    return;
                }
                let d = self.m.consts.get(c);
                self.ty(d.ty());
                match d {
                    ConstData::Array { elems, .. }
                    | ConstData::Struct { elems, .. }
                    | ConstData::Vector { elems, .. } => elems.iter().for_each(|&e| self.value(e)),
                    ConstData::Gep { source, ops, .. } => {
                        self.ty(*source);
                        ops.iter().for_each(|&e| self.value(e));
                    }
                    ConstData::Cast { val, .. } => self.value(*val),
                    ConstData::Binary { lhs, rhs, .. } => {
                        self.value(*lhs);
                        self.value(*rhs);
                    }
                    ConstData::DsoLocalEquivalent(g) | ConstData::NoCfi(g) => self.value(*g),
                    _ => {}
                }
            }
            _ => {}
        }
    }

    fn md(&mut self, root: MetadataId) {
        let mut stack = vec![root];
        while let Some(n) = stack.pop() {
            if std::mem::replace(&mut self.seen_md[n.index()], true) {
                continue;
            }
            let node = &self.m.md[n.index()];
            let mut kids = Vec::new();
            match node {
                MdNode::Tuple { ops, .. } => {
                    for &o in ops.iter().flatten() {
                        match &self.m.md[o.index()] {
                            MdNode::Value(_, v) => {
                                // Constants are incorporated in place.
                                let v = *v;
                                for k in kids.drain(..).rev() {
                                    stack.push(k);
                                }
                                self.flush(&mut stack);
                                self.value(v);
                            }
                            MdNode::String(_) => {}
                            _ => kids.push(o),
                        }
                    }
                }
                MdNode::Specialized { .. } => md_children(node, |c| kids.push(c)),
                _ => {}
            }
            for k in kids.into_iter().rev() {
                stack.push(k);
            }
        }
    }

    fn flush(&mut self, stack: &mut Vec<MetadataId>) {
        // Depth-first: process pending child nodes before a later constant
        // operand, as the recursive TypeFinder does.
        while let Some(n) = stack.pop() {
            self.md(n);
        }
    }
}

/// Adapter from `io::Write` to `fmt::Write` that keeps the first I/O error.
struct IoAdapter<'w, W: io::Write + ?Sized> {
    inner: &'w mut W,
    error: Option<io::Error>,
}

impl<W: io::Write + ?Sized> fmt::Write for IoAdapter<'_, W> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.inner.write_all(s.as_bytes()).map_err(|e| {
            self.error = Some(e);
            fmt::Error
        })
    }
}

impl Module {
    /// Print the module as LLVM 19 textual IR.
    ///
    /// # Errors
    ///
    /// Returns the writer's I/O error.
    pub fn print<W: io::Write + ?Sized>(&self, w: &mut W) -> io::Result<()> {
        let mut a = IoAdapter { inner: w, error: None };
        match Printer::new(self).module(&mut a) {
            Ok(()) => Ok(()),
            Err(fmt::Error) => Err(a.error.unwrap_or_else(|| io::Error::other("formatting error"))),
        }
    }

    /// Print the module into a `fmt::Write` sink.
    ///
    /// # Errors
    ///
    /// Returns the sink's error.
    pub fn write_to<W: fmt::Write>(&self, w: &mut W) -> fmt::Result {
        Printer::new(self).module(w)
    }

    /// The module as LLVM 19 textual IR.
    pub fn to_text(&self) -> String {
        let mut s = String::new();
        // Writing to a String cannot fail.
        let _ = self.write_to(&mut s);
        s
    }
}

impl fmt::Display for Module {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.write_to(f)
    }
}
