//! The textual IR printer (`spec/llvm.md` §7).
//!
//! Output follows LLVM 19's `AsmWriter` so that `llvm-as` accepts it and so
//! that, after normalization through `opt -S`, it matches what clang emits:
//! section order, `#N` attribute-group numbering, `!N` metadata numbering
//! and `%N` local numbering are computed the way LLVM's `SlotTracker` and
//! `TypeFinder` compute them. Printing allocates only per-module and
//! per-function side tables, never per instruction.

use std::cell::{Cell, RefCell};
use std::fmt::{self, Write as _};
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

/// The printer's sink: one `String` (WP-654). Every token is appended as a
/// whole `&str`; [`Module::print`] and [`Module::write_to`] hand the text
/// to their writer in chunks.
type Out<'w> = &'w mut String;

/// Bytes LLVM's `PrintLLVMName` prints without quoting: `[A-Za-z0-9._-]`.
static NAME_OK: [bool; 256] = {
    let mut t = [false; 256];
    let mut c = 0;
    while c < 256 {
        let b = c as u8;
        t[c] = b.is_ascii_alphanumeric() || b == b'-' || b == b'.' || b == b'_';
        c += 1;
    }
    t
};

/// Whether `name` must be printed quoted (`PrintLLVMName`).
pub(crate) fn name_needs_quotes(name: &str) -> bool {
    let b = name.as_bytes();
    b.first().is_some_and(u8::is_ascii_digit) || !b.iter().all(|&c| NAME_OK[c as usize])
}

/// Write `prefix` + `name`, quoting and escaping as LLVM's `PrintLLVMName`.
pub(crate) fn write_llvm_name<W: fmt::Write + ?Sized>(
    w: &mut W,
    prefix: char,
    name: &str,
) -> fmt::Result {
    write_name_quoted(w, prefix, name, name_needs_quotes(name))
}

/// [`write_llvm_name`] with the quoting decision already made.
#[inline]
fn write_name_quoted<W: fmt::Write + ?Sized>(
    w: &mut W,
    prefix: char,
    name: &str,
    quoted: bool,
) -> fmt::Result {
    if prefix != '\0' {
        w.write_char(prefix)?;
    }
    if quoted {
        w.write_char('"')?;
        write_escaped(w, name.as_bytes())?;
        w.write_char('"')
    } else {
        w.write_str(name)
    }
}

const HEX: &[u8; 16] = b"0123456789ABCDEF";

/// `\XX`: two upper-case hex digits.
fn write_hex_escape<W: fmt::Write + ?Sized>(w: &mut W, c: u8) -> fmt::Result {
    w.write_char('\\')?;
    w.write_char(char::from(HEX[usize::from(c >> 4)]))?;
    w.write_char(char::from(HEX[usize::from(c & 15)]))
}

/// `bytes`, known to be printable ASCII, as one `&str` token.
fn write_ascii_run<W: fmt::Write + ?Sized>(w: &mut W, bytes: &[u8]) -> fmt::Result {
    if bytes.is_empty() {
        return Ok(());
    }
    w.write_str(std::str::from_utf8(bytes).map_err(|_| fmt::Error)?)
}

/// Whether `printEscapedString` prints `c` as itself.
#[inline]
fn is_verbatim(c: u8) -> bool {
    (0x20..0x7f).contains(&c) && c != b'"' && c != b'\\'
}

/// LLVM's `printEscapedString`: printable ASCII except `"` and `\` verbatim,
/// `\` as `\\`, everything else as `\XX`. Verbatim runs are written as one
/// token.
pub(crate) fn write_escaped<W: fmt::Write + ?Sized>(w: &mut W, bytes: &[u8]) -> fmt::Result {
    let mut start = 0;
    for (i, &c) in bytes.iter().enumerate() {
        if is_verbatim(c) {
            continue;
        }
        write_ascii_run(w, &bytes[start..i])?;
        if c == b'\\' {
            w.write_str("\\\\")?;
        } else {
            write_hex_escape(w, c)?;
        }
        start = i + 1;
    }
    write_ascii_run(w, &bytes[start..])
}

/// LLVM's `printMetadataIdentifier` (named metadata, attachment kinds).
fn write_md_ident(w: Out<'_>, name: &str) -> fmt::Result {
    let b = name.as_bytes();
    let mut start = 0;
    for (i, &c) in b.iter().enumerate() {
        let ok = if i == 0 { c.is_ascii_alphabetic() } else { c.is_ascii_alphanumeric() }
            || matches!(c, b'-' | b'$' | b'.' | b'_');
        if !ok {
            write_ascii_run(w, &b[start..i])?;
            write_hex_escape(w, c)?;
            start = i + 1;
        }
    }
    write_ascii_run(w, &b[start..])
}

/// The decimal digits of `v`, as `itoa` writes them (no `fmt` machinery).
pub(crate) fn write_u64<W: fmt::Write + ?Sized>(w: &mut W, mut v: u64) -> fmt::Result {
    if v < 10 {
        return w.write_char(char::from(b'0' + v as u8));
    }
    let mut buf = [0u8; 20];
    let mut i = buf.len();
    while v >= 100 {
        let d = (v % 100) as usize * 2;
        v /= 100;
        i -= 2;
        buf[i..i + 2].copy_from_slice(&DIGITS2[d..d + 2]);
    }
    if v >= 10 {
        let d = v as usize * 2;
        i -= 2;
        buf[i..i + 2].copy_from_slice(&DIGITS2[d..d + 2]);
    } else {
        i -= 1;
        buf[i] = b'0' + v as u8;
    }
    write_ascii_run(w, &buf[i..])
}

/// `00`..`99`, two bytes each.
static DIGITS2: [u8; 200] = {
    let mut t = [0u8; 200];
    let mut n = 0;
    while n < 100 {
        t[2 * n] = b'0' + (n / 10) as u8;
        t[2 * n + 1] = b'0' + (n % 10) as u8;
        n += 1;
    }
    t
};

/// `v` in decimal, with a leading `-` when negative.
pub(crate) fn write_i64<W: fmt::Write + ?Sized>(w: &mut W, v: i64) -> fmt::Result {
    if v < 0 {
        w.write_char('-')?;
    }
    write_u64(w, v.unsigned_abs())
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
            match i64::try_from(s) {
                Ok(v) => write_i64(w, v),
                Err(_) => write!(w, "{s}"),
            }
        }
        IntValue::Big(words) => write_signed_words(w, words, bits),
    }
}

fn linkage_kw(w: Out<'_>, l: Linkage) -> fmt::Result {
    let k = l.keyword();
    if k.is_empty() {
        Ok(())
    } else {
        w.write_str(k)?;
        w.write_char(' ')
    }
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
        Some(n) => {
            w.write_str(", align ")?;
            write_u64(w, n)
        }
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
            w.write_str("i32 ")?;
            write_u64(w, u64::from(e))?;
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
    /// The printed form of each type and inline attribute set, filled on
    /// first use (WP-654).
    cache: RefCell<TextCache>,
    /// Per function / global: whether its name prints quoted
    /// ([`Quote`]), decided on first use.
    func_quote: Vec<Cell<Quote>>,
    global_quote: Vec<Cell<Quote>>,
}

/// A name's quoting, decided once per module.
#[derive(Copy, Clone, PartialEq, Eq)]
enum Quote {
    Unknown,
    /// `@name`.
    Plain,
    /// `@"name"`, every byte printed as is.
    Quoted,
    /// `@"name"` with `\XX` escapes.
    Escaped,
}

/// Text of types and attribute sets as printed, in one arena:
/// `ty[id]`/`attr[id]` is the byte range, `(NO_SLOT, _)` before first use.
struct TextCache {
    arena: String,
    ty: Vec<(u32, u32)>,
    attr: Vec<(u32, u32)>,
}

impl TextCache {
    /// Append the cached text of entry `i` of `table` to `w`, printing it
    /// into the arena with `print` on first use.
    fn emit(
        &mut self,
        w: &mut String,
        pick: fn(&mut TextCache) -> &mut Vec<(u32, u32)>,
        i: usize,
        print: impl FnOnce(&mut String) -> fmt::Result,
    ) -> fmt::Result {
        if let Some(&(a, b)) = pick(self).get(i)
            && a != NO_SLOT
        {
            w.push_str(&self.arena[a as usize..b as usize]);
            return Ok(());
        }
        let a = self.arena.len();
        print(&mut self.arena)?;
        let b = self.arena.len();
        w.push_str(&self.arena[a..b]);
        if let (Ok(a), Ok(b)) = (u32::try_from(a), u32::try_from(b))
            && a != NO_SLOT
            && b != NO_SLOT
            && let Some(e) = pick(self).get_mut(i)
        {
            *e = (a, b);
        }
        Ok(())
    }
}

/// `@name`, or `@N` for an unnamed global; `q` caches the quoting.
fn global_name(w: Out<'_>, name: &str, slot: u32, q: &Cell<Quote>) -> fmt::Result {
    if name.is_empty() {
        w.write_char('@')?;
        return write_u64(w, u64::from(slot));
    }
    let mut quote = q.get();
    if quote == Quote::Unknown {
        quote = if !name_needs_quotes(name) {
            Quote::Plain
        } else if name.bytes().all(is_verbatim) {
            Quote::Quoted
        } else {
            Quote::Escaped
        };
        q.set(quote);
    }
    match quote {
        Quote::Plain | Quote::Unknown => {
            w.push('@');
            w.push_str(name);
        }
        Quote::Quoted => {
            w.push_str("@\"");
            w.push_str(name);
            w.push('"');
        }
        Quote::Escaped => write_name_quoted(w, '@', name, true)?,
    }
    Ok(())
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
            global_slot: vec![NO_SLOT; m.globals.len()],
            alias_slot: Vec::with_capacity(m.aliases.len()),
            func_slot: vec![NO_SLOT; m.funcs.len()],
            cur: None,
            local: LocalSlots::default(),
            cache: RefCell::new(TextCache {
                arena: String::new(),
                ty: vec![(NO_SLOT, 0); m.types.len()],
                attr: vec![(NO_SLOT, 0); m.attrs.len()],
            }),
            func_quote: vec![Cell::new(Quote::Unknown); m.funcs.len()],
            global_quote: vec![Cell::new(Quote::Unknown); m.globals.len()],
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
        for &g in &m.global_order {
            p.global_slot[g.index()] = next(m.globals[g.index()].name.is_empty());
        }
        for a in &m.aliases {
            p.alias_slot.push(next(a.name.is_empty()));
        }
        for &f in &m.func_order {
            p.func_slot[f.index()] = next(m.funcs[f.index()].name.is_empty());
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
        for f in m.ordered_funcs() {
            self.add_attr_slot(f.attrs.func);
        }
        for f in m.ordered_funcs() {
            for &bb in &f.layout {
                for &i in &f.blocks[bb.index()].insts {
                    if let InstKind::Call(c) | InstKind::Invoke(c) | InstKind::CallBr(c) =
                        f.insts[i.index()].kind
                    {
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
        for g in m.ordered_globals() {
            for &(_, md) in &g.md {
                self.add_md_slot(md, &mut stack);
            }
        }
        for (_, ops) in &m.named_md {
            for &md in ops {
                self.add_md_slot(md, &mut stack);
            }
        }
        for f in m.ordered_funcs() {
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
        {
            let c = self.cache.borrow();
            if let Some(&(a, b)) = c.ty.get(ty.index())
                && a != NO_SLOT
            {
                w.push_str(&c.arena[a as usize..b as usize]);
                return Ok(());
            }
        }
        let types = &self.m.types;
        self.cache.borrow_mut().emit(w, |c| &mut c.ty, ty.index(), |a| types.write(a, ty))
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
                Some(&s) if s != NO_SLOT => {
                    w.write_char('%')?;
                    write_u64(w, u64::from(s))
                }
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
                        s => {
                            w.write_char('%')?;
                            write_u64(w, u64::from(s))
                        }
                    },
                    None => w.write_str("<badref>"),
                }
            }
            ValueRef::Arg(n) => {
                let Some(f) = self.func() else { return w.write_str("<badref>") };
                match f.arg_names.get(n as usize) {
                    Some(name) if !name.is_empty() => write_llvm_name(w, '%', name),
                    Some(_) => {
                        w.write_char('%')?;
                        write_u64(w, u64::from(self.local.args[n as usize]))
                    }
                    None => w.write_str("<badref>"),
                }
            }
            ValueRef::Block(b) => match self.func() {
                Some(f) => Self::block_ref(w, f, &self.local.blocks, b),
                None => w.write_str("<badref>"),
            },
            ValueRef::Const(c) => self.constant(w, m.consts.get(c)),
            ValueRef::Func(f) => {
                let r = m.resolve(v);
                if r != v {
                    return self.operand(w, r);
                }
                let q = &self.func_quote[f.index()];
                global_name(w, &m.funcs[f.index()].name, self.func_slot[f.index()], q)
            }
            ValueRef::Global(g) => {
                let r = m.resolve(v);
                if r != v {
                    return self.operand(w, r);
                }
                let q = &self.global_quote[g.index()];
                global_name(w, &m.globals[g.index()].name, self.global_slot[g.index()], q)
            }
            ValueRef::Alias(a) => {
                let q = &Cell::new(Quote::Unknown);
                global_name(w, &m.aliases[a.index()].name, self.alias_slot[a.index()], q)
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

    /// `T v`.
    fn typed(&self, w: Out<'_>, v: Value) -> fmt::Result {
        self.ty(w, self.value_type(v))?;
        w.write_char(' ')?;
        self.operand(w, v)
    }

    /// `{ T a, … }`, or `<{ … }>` for a packed struct type.
    fn struct_const(&self, w: Out<'_>, ty: TypeId, elems: &[Value]) -> fmt::Result {
        let packed = self.m.types.struct_is_packed(ty);
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

    /// A scalable splat constant: `splat (T v)` for LLVM 20+; LLVM 19's
    /// `shufflevector (<vscale x N x T> insertelement (<vscale x N x T>
    /// poison, T v, i64 0), <vscale x N x T> poison, <vscale x N x i32>
    /// zeroinitializer)` before.
    fn splat(&self, w: Out<'_>, ty: TypeId, elem: Value) -> fmt::Result {
        let m = self.m;
        if m.ir_version >= 20 {
            w.write_str("splat (")?;
            self.typed(w, elem)?;
            return w.write_char(')');
        }
        let n = m.types.vector_info(ty).map_or(0, |v| v.1);
        w.write_str("shufflevector (")?;
        self.ty(w, ty)?;
        w.write_str(" insertelement (")?;
        self.ty(w, ty)?;
        w.write_str(" poison, ")?;
        self.typed(w, elem)?;
        w.write_str(", i64 0), ")?;
        self.ty(w, ty)?;
        write!(w, " poison, <vscale x {n} x i32> zeroinitializer)")
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
                // LLVM 20+'s AsmWriter writes a vector whose lanes are one
                // integer or floating-point constant as `splat (T v)`.
                if let Some(&first) = elems.first()
                    && m.ir_version >= 20
                    && elems.iter().all(|&e| e == first)
                    && first.as_const().is_some_and(|c| {
                        matches!(m.consts.get(c), ConstData::Int { .. } | ConstData::Float { .. })
                    })
                {
                    w.write_str("splat (")?;
                    self.typed(w, first)?;
                    return w.write_char(')');
                }
                w.write_char('<')?;
                self.typed_list(w, elems)?;
                w.write_char('>')
            }
            ConstData::Splat { ty, elem } => self.splat(w, *ty, *elem),
            ConstData::Struct { ty, elems } => self.struct_const(w, *ty, elems),
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
                w.write_str(op.name())?;
                w.write_str(" (")?;
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
                s => {
                    w.write_char('!')?;
                    write_u64(w, u64::from(s))
                }
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
        let m = self.m;
        self.cache.borrow_mut().emit(
            w,
            |c| &mut c.attr,
            s.index(),
            |w| {
                for (i, a) in m.attrs.get(s).iter().enumerate() {
                    if i > 0 {
                        w.write_char(' ')?;
                    }
                    a.write(w, &m.types)?;
                }
                Ok(())
            },
        )
    }

    fn attr_group_ref(&self, w: Out<'_>, s: AttrSetId) -> fmt::Result {
        if s != AttrSetId::EMPTY {
            w.write_str(" #")?;
            write_u64(w, u64::from(self.attr_slot[s.index()]))?;
        }
        Ok(())
    }

    // ---- module ---------------------------------------------------------

    /// Print the module into `w`. When `flush` is given, `w` is handed to
    /// it (and cleared) whenever it passes [`CHUNK`] bytes at a global,
    /// function or metadata boundary, so an `io::Write`/`fmt::Write` sink
    /// sees a few large writes instead of one call per token.
    fn module(&mut self, w: Out<'_>, mut flush: Option<Flush<'_>>) -> fmt::Result {
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
        if !m.global_order.is_empty() {
            w.write_char('\n')?;
        }
        for g in &m.global_order {
            self.global(w, g.index())?;
            w.write_char('\n')?;
            chunk(w, &mut flush)?;
        }
        if !m.aliases.is_empty() {
            w.write_char('\n')?;
        }
        // LLVM's `AsmWriter` prints every alias, then every ifunc.
        for ifunc in [false, true] {
            for i in 0..m.aliases.len() {
                if m.aliases[i].ifunc == ifunc {
                    self.alias(w, i)?;
                }
            }
        }
        for &f in &m.func_order {
            w.write_char('\n')?;
            self.function(w, f)?;
            chunk(w, &mut flush)?;
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
                w.write_char('!')?;
                write_u64(w, n as u64)?;
                w.write_str(" = ")?;
                self.md_body(w, &m.md[md.index()])?;
                w.write_char('\n')?;
                chunk(w, &mut flush)?;
            }
        }
        if let Some(f) = flush.as_mut() {
            f(w)?;
            w.clear();
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
            .ordered_funcs()
            .filter_map(|f| f.comdat)
            .chain(m.ordered_globals().filter_map(|g| g.comdat));
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
        global_name(w, &g.name, self.global_slot[i], &self.global_quote[i])?;
        w.write_str(" = ")?;
        if g.init.is_none() && g.linkage == Linkage::External {
            w.write_str("external ")?;
        }
        linkage_kw(w, g.linkage)?;
        write_dso_vis_dll(w, g.linkage, g.dso_local, g.visibility, g.dll_storage)?;
        write_tls(w, g.thread_local)?;
        let ua = unnamed_addr_kw(g.unnamed_addr);
        if !ua.is_empty() {
            w.write_str(ua)?;
            w.write_char(' ')?;
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
            w.write_str(", align ")?;
            write_u64(w, a)?;
        }
        self.attachments(w, g.md.iter().copied(), ", ")
    }

    fn alias(&self, w: Out<'_>, i: usize) -> fmt::Result {
        let a = &self.m.aliases[i];
        global_name(w, &a.name, self.alias_slot[i], &Cell::new(Quote::Unknown))?;
        w.write_str(" = ")?;
        linkage_kw(w, a.linkage)?;
        write_dso_vis_dll(w, a.linkage, a.dso_local, a.visibility, a.dll_storage)?;
        if a.ifunc {
            w.write_str("ifunc ")?;
        } else {
            write_tls(w, a.thread_local)?;
            let ua = unnamed_addr_kw(a.unnamed_addr);
            if !ua.is_empty() {
                w.write_str(ua)?;
                w.write_char(' ')?;
            }
            w.write_str("alias ")?;
        }
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
        let q = &self.func_quote[fid.index()];
        global_name(w, &f.name, self.func_slot[fid.index()], q)?;
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
            w.write_char(' ')?;
            w.write_str(ua)?;
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
            w.write_str(" align ")?;
            write_u64(w, a)?;
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
                    w.write_char('\n')?;
                    write_u64(w, u64::from(self.local.blocks[bb.index()]))?;
                    w.write_char(':')?;
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
                w.write_str("icmp ")?;
                w.write_str(pred.name())?;
                self.same_typed(w, &[lhs, rhs])?;
            }
            InstKind::FCmp { pred, fmf, lhs, rhs } => {
                w.write_str("fcmp")?;
                write_fmf(w, fmf)?;
                w.write_char(' ')?;
                w.write_str(pred.name())?;
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
                w.write_char(' ')?;
                w.write_str(success.name())?;
                w.write_char(' ')?;
                w.write_str(failure.name())?;
                write_align(w, align)?;
            }
            InstKind::AtomicRmw { op, ptr, val, align, ordering, volatile, scope } => {
                w.write_str("atomicrmw")?;
                if volatile {
                    w.write_str(" volatile")?;
                }
                w.write_char(' ')?;
                w.write_str(op.name())?;
                w.write_char(' ')?;
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
                    w.write_str(", ")?;
                    write_u64(w, u64::from(*i))?;
                }
            }
            InstKind::InsertValue { agg, val, idx } => {
                w.write_str("insertvalue ")?;
                self.typed(w, agg)?;
                w.write_str(", ")?;
                self.typed(w, val)?;
                for i in f.idx(idx) {
                    w.write_str(", ")?;
                    write_u64(w, u64::from(*i))?;
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
            InstKind::Call(c) | InstKind::Invoke(c) | InstKind::CallBr(c) => {
                let call = &f.calls[c as usize];
                w.write_str(match inst.kind {
                    InstKind::Invoke(_) => "invoke",
                    InstKind::CallBr(_) => "callbr",
                    _ => "call",
                })?;
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
                if let (InstKind::CallBr(_), Some(n)) = (inst.kind, call.normal) {
                    w.write_str("\n          to label ")?;
                    self.operand(w, Value::block(n))?;
                    w.write_str(" [")?;
                    for (i, &b) in call.indirect.iter().enumerate() {
                        if i > 0 {
                            w.write_str(", ")?;
                        }
                        w.write_str("label ")?;
                        self.operand(w, Value::block(b))?;
                    }
                    w.write_char(']')?;
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
        w.write_char(' ')?;
        w.write_str(o.name())
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
        for g in m.ordered_globals() {
            t.ty(g.value_type);
            if let Some(i) = g.init {
                t.value(i);
            }
        }
        for a in &m.aliases {
            t.ty(a.value_type);
            t.value(a.aliasee);
        }
        for f in m.ordered_funcs() {
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
            InstKind::Call(c) | InstKind::Invoke(c) | InstKind::CallBr(c) => {
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
            InstKind::Call(c) | InstKind::Invoke(c) | InstKind::CallBr(c) => {
                self.attrs(&f.calls[c as usize].attrs);
            }
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
                    ConstData::Splat { elem, .. } => self.value(*elem),
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

/// Receives a printed chunk.
type Flush<'f> = &'f mut dyn FnMut(&str) -> fmt::Result;

/// Bytes buffered before a chunk is handed to the sink.
const CHUNK: usize = 1 << 16;

#[inline]
fn chunk(w: &mut String, flush: &mut Option<Flush<'_>>) -> fmt::Result {
    if let Some(f) = flush.as_mut()
        && w.len() >= CHUNK
    {
        f(w)?;
        w.clear();
    }
    Ok(())
}

impl Module {
    /// Print the module as LLVM 19 textual IR.
    ///
    /// # Errors
    ///
    /// Returns the writer's I/O error.
    pub fn print<W: io::Write + ?Sized>(&self, w: &mut W) -> io::Result<()> {
        let mut error = None;
        let mut buf = String::with_capacity(CHUNK + CHUNK / 4);
        let mut f = |s: &str| {
            w.write_all(s.as_bytes()).map_err(|e| {
                error = Some(e);
                fmt::Error
            })
        };
        match Printer::new(self).module(&mut buf, Some(&mut f)) {
            Ok(()) => Ok(()),
            Err(fmt::Error) => Err(error.unwrap_or_else(|| io::Error::other("formatting error"))),
        }
    }

    /// Print the module into a `fmt::Write` sink.
    ///
    /// # Errors
    ///
    /// Returns the sink's error.
    pub fn write_to<W: fmt::Write + ?Sized>(&self, w: &mut W) -> fmt::Result {
        let mut buf = String::with_capacity(CHUNK + CHUNK / 4);
        Printer::new(self).module(&mut buf, Some(&mut |s: &str| w.write_str(s)))
    }

    /// A rough upper-middle estimate of the printed size, used to pre-size
    /// [`Module::to_text`]'s buffer (an under-estimate only costs a
    /// `realloc`; capacity that is never written is never resident).
    fn text_size_hint(&self) -> usize {
        let insts: usize = self.funcs.iter().map(|f| f.insts.len()).sum();
        4096 + insts * 40 + self.funcs.len() * 96 + self.globals.len() * 96 + self.md.len() * 48
    }

    /// The module as LLVM 19 textual IR.
    pub fn to_text(&self) -> String {
        let mut s = String::with_capacity(self.text_size_hint());
        // Writing to a String cannot fail.
        let _ = Printer::new(self).module(&mut s, None);
        s
    }
}

impl fmt::Display for Module {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.write_to(f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn integers_match_display() {
        let edges =
            [0, 1, 9, 10, 99, 100, 101, 999, 1000, 4096, i64::MAX, -1, -10, -4096, i64::MIN];
        for v in (0..20_000).chain(edges) {
            let mut s = String::new();
            write_i64(&mut s, v).unwrap();
            assert_eq!(s, v.to_string());
        }
        let mut s = String::new();
        write_u64(&mut s, u64::MAX).unwrap();
        assert_eq!(s, u64::MAX.to_string());
    }

    #[test]
    fn escaping_writes_runs_and_escapes() {
        let mut s = String::new();
        write_escaped(&mut s, b"ab\"c\\d\n\xffe").unwrap();
        assert_eq!(s, "ab\\22c\\\\d\\0A\\FFe");
        let mut s = String::new();
        write_llvm_name(&mut s, '@', "_Z1fv").unwrap();
        write_llvm_name(&mut s, '%', "a b").unwrap();
        write_llvm_name(&mut s, '@', "0x").unwrap();
        write_llvm_name(&mut s, '@', "a.-_9").unwrap();
        write_llvm_name(&mut s, '@', "$x").unwrap();
        write_llvm_name(&mut s, '%', "").unwrap();
        assert_eq!(s, "@_Z1fv%\"a b\"@\"0x\"@a.-_9@\"$x\"%");
    }

    #[test]
    fn every_entry_point_prints_the_same_text() {
        let mut m = Module::new("m");
        let i32t = m.types.int(32);
        for n in 0..10_000 {
            let g = m.add_global(&format!("g{n}"), i32t);
            let c = m.const_int(i32t, i128::from(n));
            m.global_mut(g).init = Some(c);
        }
        for name in ["a$b", "a\"b", "0x", "plain"] {
            let g = m.add_global(name, i32t);
            let ptr = m.types.ptr(0);
            let user = m.add_global(&format!("p.{name}"), ptr);
            m.global_mut(user).init = Some(Value::global(g));
        }
        let text = m.to_text();
        assert!(text.len() > 2 * CHUNK);
        for want in
            ["@\"a$b\" = ", "@\"a\\22b\" = ", "@\"p.a$b\" = global ptr @\"a$b\"", "@\"0x\" = "]
        {
            assert!(text.contains(want), "{want}");
        }
        assert!(text.contains("@p.plain = global ptr @plain"));
        assert_eq!(m.to_string(), text);
        let mut s = String::new();
        m.write_to(&mut s).unwrap();
        assert_eq!(s, text);
        let mut v = Vec::new();
        m.print(&mut v).unwrap();
        assert_eq!(v, text.as_bytes());
    }
}
