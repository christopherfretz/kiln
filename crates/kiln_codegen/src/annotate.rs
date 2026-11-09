//! `[[clang::annotate("str", args...)]]` lowering (spec/codegen.md
//! "Annotations", spec/landed/codegen/WP-1113.md): clang
//! `CodeGenModule::AddGlobalAnnotations` / `EmitAnnotateAttr` /
//! `EmitAnnotationArgs` and `CodeGenFunction::EmitVarAnnotations` /
//! `EmitFieldAnnotations`.
//!
//! - A defined global variable, a static local and a function append one
//!   `{ ptr gv, ptr str, ptr file, i32 line, ptr args }` element per
//!   attribute to `@llvm.global.annotations` (section `llvm.metadata`).
//!   Variables are annotated when their definition is emitted; functions
//!   are remembered by mangled name when first created and annotated at
//!   the end of the module with their latest redeclaration.
//! - A local variable or parameter gets `call void
//!   @llvm.var.annotation.p0.p0(ptr addr, ...)` after its storage is set
//!   up.
//! - Every access to an annotated non-static data member that is not a
//!   bit-field wraps its address in `@llvm.ptr.annotation.p0.p0` calls.
//!
//! Strings (annotation strings and file names, one cache) are private
//! `unnamed_addr` constants `.str` in `llvm.metadata` without alignment;
//! the arguments are an anonymous struct `.args`, cached by clang's
//! `APValue::Profile` of the argument values.

use kiln_ast::attr::AttrKind as AstAttr;
use kiln_ast::consts::{ConstValue, NullKind, PtrBase, Scalar};
use kiln_ast::expr::ExprKind;
use kiln_ast::ids::RawId;
use kiln_ast::{AttrId, ConstValueId, DeclId, ExprId, ListId, QualType};
use kiln_llvm::{GlobalId, Linkage, TypeId as LTy, UnnamedAddr, Value};
use kiln_source::Loc;
use kiln_source::fx::{FxHashMap, FxHashSet};

use crate::fir::V;
use crate::module::{Cg, R, err, gval};

/// The section every annotation constant lives in.
const SECTION: &str = "llvm.metadata";

/// Per-module annotation state.
#[derive(Default)]
pub(crate) struct Annotations {
    /// Annotation strings and file names by contents (clang
    /// `AnnotationStrings`).
    strings: FxHashMap<Vec<u8>, GlobalId>,
    /// `.args` structs by the profile of their values (clang
    /// `AnnotationArgs`).
    args: FxHashMap<Vec<Part>, GlobalId>,
    /// The `@llvm.global.annotations` elements so far.
    entries: Vec<Value>,
    /// Functions to annotate at the end of the module, by mangled name,
    /// in creation order (clang `DeferredAnnotations`).
    fns: Vec<(String, DeclId)>,
    /// The names in `fns`.
    fn_seen: FxHashSet<String>,
}

/// One component of clang's `APValue::Profile` of an annotation
/// argument.
#[derive(Clone, PartialEq, Eq, Hash)]
enum Part {
    /// An integer (`bool` included) as 32-bit chunks of its bits: the
    /// width matters, the signedness does not (`1`, `1u`, `true` and
    /// `(char)1` share a struct, `1L` does not).
    Int(Vec<u32>),
    /// A floating-point value as 32-bit chunks of its encoding.
    Float(Vec<u32>),
    /// A null pointer (false) or null member pointer (true).
    Null(bool),
    /// A pointer: base, byte offset, one-past-the-end.
    Ptr(PtrBase, u64, bool),
    /// An aggregate: type, bytes and the profiles of its relocations.
    Agg(u32, Vec<u8>, Vec<(u32, Vec<Part>)>),
    /// Any other value, by its interned id.
    Other(u32),
}

/// `bits` as `n_bits.div_ceil(32)` 32-bit chunks, least significant
/// first (clang `profileIntValue`).
fn chunks(words: &[u64], n_bits: u32) -> Vec<u32> {
    (0..n_bits.div_ceil(32))
        .map(|i| {
            let w = words.get((i / 2) as usize).copied().unwrap_or(0);
            (w >> (32 * (i % 2))) as u32
        })
        .collect()
}

impl Cg<'_> {
    /// Whether the translation unit has any `annotate` attribute.
    fn ann_on(&self) -> bool {
        self.ast.attrs.has_kind(AstAttr::ANNOTATE)
    }

    /// The contents of `annotate` attribute `a`'s string (no terminator).
    fn ann_str(&self, a: AttrId) -> Vec<u8> {
        let ast = self.ast;
        let Some(mut e) = ExprId::from_raw(ast.attrs.get(a).a) else { return Vec::new() };
        for _ in 0..64 {
            match ast.exprs.get(e).kind() {
                ExprKind::StringLiteral => {
                    return ast.strings.bytes(ast.exprs.string_lit(e)).to_vec();
                }
                ExprKind::ParenExpr => e = ast.exprs.operand(e),
                _ => break,
            }
        }
        Vec::new()
    }

    /// The `annotate` attributes declaration `d` itself carries, in order.
    fn ann_own(&self, d: DeclId) -> Vec<AttrId> {
        let ast = self.ast;
        ast.lists
            .iter::<AttrId>(ast.attrs.of_decl(d))
            .filter(|&a| ast.attrs.get(a).kind == AstAttr::ANNOTATE)
            .collect()
    }

    /// The `annotate` attributes of `d` with those it inherits (clang
    /// `mergeDeclAttributes` / `DeclHasAttr`): walking the redeclarations
    /// from the first to `d`, each one inherits, ahead of its own, every
    /// attribute of the previous one whose string neither its own nor an
    /// already inherited one has (arguments are not compared).
    pub(crate) fn ann_attrs(&self, d: DeclId) -> Vec<AttrId> {
        let ast = self.ast;
        let mut chain: Vec<DeclId> = ast.redecls_of(d).collect();
        chain.reverse();
        let Some(end) = chain.iter().position(|&x| x == d) else { return self.ann_own(d) };
        let mut eff: Vec<AttrId> = Vec::new();
        for &r in &chain[..=end] {
            let own = self.ann_own(r);
            let mut seen: Vec<Vec<u8>> = own.iter().map(|&a| self.ann_str(a)).collect();
            let mut next = Vec::with_capacity(eff.len() + own.len());
            for a in eff {
                let s = self.ann_str(a);
                if !seen.contains(&s) {
                    seen.push(s);
                    next.push(a);
                }
            }
            next.extend(own);
            eff = next;
        }
        eff
    }

    /// The `.str` constant holding `bytes` and a terminator (clang
    /// `EmitAnnotationString`).
    fn ann_string(&mut self, bytes: &[u8]) -> Value {
        if let Some(&g) = self.ann.strings.get(bytes) {
            return gval(g);
        }
        let mut b = bytes.to_vec();
        b.push(0);
        let init = self.m.const_bytes(&b);
        let ty = self.m.const_type(init);
        let g = self.m.add_global(".str", ty);
        let _ = self.m.set_initializer(g, Some(init));
        let gv = self.m.global_mut(g);
        gv.linkage = Linkage::Private;
        gv.unnamed_addr = UnnamedAddr::Global;
        gv.is_constant = true;
        gv.section = Some(SECTION.into());
        self.ann.strings.insert(bytes.to_vec(), g);
        gval(g)
    }

    /// The file name and line constants of `loc` (clang
    /// `EmitAnnotationUnit` / `EmitAnnotationLineNo`: the presumed
    /// location, `#line` applied).
    fn ann_unit_line(&mut self, loc: Loc) -> (Value, Value) {
        let (name, line) = match self.sm.and_then(|sm| sm.presumed_line_col(loc)) {
            Some(p) => (p.file_name.as_bytes().to_vec(), p.line),
            None => (Vec::new(), 0),
        };
        let file = self.ann_string(&name);
        (file, self.m.const_int(LTy::I32, i128::from(line)))
    }

    /// Appends the profile of constant `c` to `out` (clang
    /// `APValue::Profile`).
    fn ann_profile(&self, c: ConstValueId, out: &mut Vec<Part>) {
        match self.ast.consts.get(c) {
            ConstValue::Scalar(Scalar::Int { bits, .. }) => {
                out.push(Part::Int(chunks(bits.words(), bits.width())));
            }
            ConstValue::Scalar(Scalar::Bool { value, .. }) => {
                out.push(Part::Int(vec![u32::from(value)]));
            }
            ConstValue::Scalar(Scalar::Float { kind, bits, .. }) => {
                let words = [bits as u64, (bits >> 64) as u64];
                out.push(Part::Float(chunks(&words, kind.bits())));
            }
            ConstValue::Scalar(Scalar::Null { kind, .. }) => {
                out.push(Part::Null(kind != NullKind::Pointer));
            }
            ConstValue::Scalar(Scalar::Pointer { base, offset, past, .. }) => {
                out.push(Part::Ptr(base, offset, past));
            }
            ConstValue::Aggregate(img) => {
                let relocs = img
                    .relocs
                    .iter()
                    .map(|r| {
                        let mut p = Vec::new();
                        self.ann_profile(r.value, &mut p);
                        (r.at, p)
                    })
                    .collect();
                out.push(Part::Agg(img.ty.raw(), img.bytes.to_vec(), relocs));
            }
            ConstValue::Scalar(_) => out.push(Part::Other(c.raw())),
        }
    }

    /// The `.args` struct of `annotate` attribute `a`, or `ptr null`
    /// without arguments (clang `EmitAnnotationArgs`): each argument's
    /// value as `emitAbstract` lowers it (`bool` is `i1`, a class its
    /// memory form), structs of equal profile shared.
    fn ann_args(&mut self, a: AttrId) -> R<Value> {
        let ast = self.ast;
        let n = *ast.attrs.get(a);
        let args: Vec<ExprId> = match ListId::from_raw(n.b) {
            Some(l) if n.b != 0 => ast.lists.iter::<ExprId>(l).collect(),
            _ => Vec::new(),
        };
        // A `void` argument has no value (sema keeps it unwrapped).
        let vals: Vec<(ExprId, ConstValueId)> = args
            .iter()
            .filter(|&&e| ast.exprs.get(e).kind() == ExprKind::ConstantExpr)
            .filter_map(|&e| ast.exprs.constant_value(e).map(|c| (e, c)))
            .collect();
        if vals.is_empty() {
            return Ok(self.m.const_null(LTy::PTR));
        }
        let mut key = Vec::with_capacity(vals.len());
        for &(_, c) in &vals {
            self.ann_profile(c, &mut key);
        }
        if let Some(&g) = self.ann.args.get(&key) {
            return Ok(gval(g));
        }
        let mut elems = Vec::with_capacity(vals.len());
        for &(e, c) in &vals {
            let ty = ast.exprs.get(e).ty();
            let v = match ast.consts.get(c) {
                ConstValue::Aggregate(_) if !self.ann_is_vector(ty) => {
                    self.const_value(c, ty, Some(e), true)?
                }
                _ => self.const_scalar_value(c, ty)?,
            };
            elems.push(v);
        }
        let tys: Vec<LTy> = elems.iter().map(|&v| self.m.const_type(v)).collect();
        let sty = self.m.types.struct_literal(&tys, false);
        let init = match self.m.const_struct(sty, &elems) {
            Ok(v) => v,
            Err(e) => return err(Some(n.loc), format!("bad annotation arguments: {e:?}")),
        };
        let g = self.m.add_global(".args", sty);
        let _ = self.m.set_initializer(g, Some(init));
        let gv = self.m.global_mut(g);
        gv.linkage = Linkage::Private;
        gv.unnamed_addr = UnnamedAddr::Global;
        gv.is_constant = true;
        gv.section = Some(SECTION.into());
        self.ann.args.insert(key, g);
        Ok(gval(g))
    }

    /// Whether `ty` lowers to an IR vector (its constant is a value).
    fn ann_is_vector(&mut self, ty: QualType) -> bool {
        matches!(self.tys.convert(ty), kiln_abi::irtype::IrType::Vector(..))
    }

    /// The string, file, line and arguments of attribute `a` on a
    /// declaration at `loc`, in clang's evaluation order.
    fn ann_operands(&mut self, a: AttrId, loc: Loc) -> R<[Value; 4]> {
        let s = self.ann_str(a);
        let s = self.ann_string(&s);
        let (file, line) = self.ann_unit_line(loc);
        let args = self.ann_args(a)?;
        Ok([s, file, line, args])
    }

    /// Appends the `@llvm.global.annotations` elements of `d`'s
    /// attributes for its global `gv` (clang `AddGlobalAnnotations`).
    pub(crate) fn add_global_annotations(&mut self, d: DeclId, gv: Value) -> R<()> {
        if !self.ann_on() {
            return Ok(());
        }
        let loc = self.ast.decls.get(d).loc();
        for a in self.ann_attrs(d) {
            let [s, file, line, args] = self.ann_operands(a, loc)?;
            let sty = self.ann_elem_ty();
            match self.m.const_struct(sty, &[gv, s, file, line, args]) {
                Ok(v) => self.ann.entries.push(v),
                Err(e) => return err(Some(loc), format!("bad annotation: {e:?}")),
            }
        }
        Ok(())
    }

    /// `{ ptr, ptr, ptr, i32, ptr }`.
    fn ann_elem_ty(&mut self) -> LTy {
        let p = LTy::PTR;
        self.m.types.struct_literal(&[p, p, p, LTy::I32, p], false)
    }

    /// Remembers the function `name` just created for `first`, to be
    /// annotated at the end of the module if a redeclaration has an
    /// `annotate` attribute (clang `DeferredAnnotations`).
    pub(crate) fn ann_note_fn(&mut self, name: &str, first: DeclId) {
        if !self.ann_on() || self.ann.fn_seen.contains(name) {
            return;
        }
        let ast = self.ast;
        if ast.redecls_of(first).all(|r| self.ann_own(r).is_empty()) {
            return;
        }
        self.ann.fn_seen.insert(name.to_owned());
        self.ann.fns.push((name.to_owned(), first));
    }

    /// `@llvm.var.annotation` calls for local variable or parameter `d`
    /// at `addr` (clang `EmitVarAnnotations`), if code is reachable.
    pub(crate) fn emit_var_annotations(&mut self, d: DeclId, addr: V) -> R<()> {
        if !self.ann_on() || self.f.fir.cur().is_none() {
            return Ok(());
        }
        let attrs = self.ann_own(d);
        if attrs.is_empty() {
            return Ok(());
        }
        let f = kiln_llvm::intrinsics::annotation(&mut self.m, false);
        let loc = self.ast.decls.get(d).loc();
        for a in attrs {
            let [s, file, line, args] = self.ann_operands(a, loc)?;
            let ops = vec![addr, V::C(s), V::C(file), V::C(line), V::C(args)];
            self.intrinsic_call(f, ops);
        }
        Ok(())
    }

    /// `addr` of field `field` wrapped in one `@llvm.ptr.annotation` call
    /// per attribute (clang `EmitFieldAnnotations`).
    pub(crate) fn emit_field_annotations(&mut self, field: DeclId, addr: V) -> R<V> {
        if !self.ann_on() {
            return Ok(addr);
        }
        let attrs = self.ann_own(field);
        if attrs.is_empty() {
            return Ok(addr);
        }
        let f = kiln_llvm::intrinsics::annotation(&mut self.m, true);
        let loc = self.ast.decls.get(field).loc();
        let mut v = addr;
        for a in attrs {
            let [s, file, line, args] = self.ann_operands(a, loc)?;
            let ops = vec![v, V::C(s), V::C(file), V::C(line), V::C(args)];
            v = self.intrinsic_call(f, ops);
        }
        Ok(v)
    }

    /// Annotates the remembered functions and emits
    /// `@llvm.global.annotations` (clang `EmitGlobalAnnotations`).
    pub(crate) fn emit_global_annotations(&mut self) -> R<()> {
        let fns = std::mem::take(&mut self.ann.fns);
        for (name, first) in fns {
            let Some(f) = self.m.get_function(&name) else { continue };
            let d = self.ast.latest(first);
            self.add_global_annotations(d, Value::func(f))?;
        }
        if self.ann.entries.is_empty() {
            return Ok(());
        }
        let entries = std::mem::take(&mut self.ann.entries);
        let ty = self.ann_elem_ty();
        self.appending_global("llvm.global.annotations", ty, &entries, Some(SECTION))
    }
}
