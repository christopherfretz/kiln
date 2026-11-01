//! Null initialization of objects whose null value is not all-zero bytes
//! (spec/codegen.md §3, "Zero-initialization"): clang's
//! `CodeGenTypes::isZeroInitializable`, `CodeGenModule::EmitNullConstant`
//! and `CodeGenFunction::EmitNullInitialization`.
//!
//! Under the Itanium ABI a null data member pointer is `-1`, so a class or
//! array that (through members, bases or elements) holds one cannot be
//! zero-initialized ([dcl.init.general]/6) with a `memset` of 0: its null
//! constant is copied from a private global instead.

use kiln_abi::irtype::IrType;
use kiln_ast::decl::{DeclKind, DefinitionBits};
use kiln_ast::types::TypeView;
use kiln_ast::{DeclId, QualType};
use kiln_llvm::{GlobalId, Linkage, TypeId as LTy, Value};

use crate::consts::ir_err;
use crate::fir::V;
use crate::func::Addr;
use crate::module::{Cg, R};

impl Cg<'_> {
    /// clang `isZeroInitializable(QualType)`: the null value of `q` is
    /// all-zero bytes.
    pub(crate) fn zero_init(&mut self, q: QualType) -> bool {
        self.tys.is_zero_initializable(q)
    }

    /// clang `CodeGenModule::EmitNullConstant`: the in-memory null value
    /// of `q` ([dcl.init.general]/6), `zeroinitializer` unless a data
    /// member pointer (`i64 -1`) is inside.
    pub(crate) fn null_const(&mut self, q: QualType) -> R<Value> {
        let t = self.mty(q);
        if self.zero_init(q) {
            return Ok(self.m.const_zero(t));
        }
        let ast = self.ast;
        match ast.types.canonical_view(q) {
            TypeView::ConstantArray(a) => {
                let n = ast.types.array_size(q).unwrap_or(0);
                let elem = self.null_const(a.elem)?;
                let et = self.m.const_type(elem);
                let elems = vec![elem; usize::try_from(n).unwrap_or(0)];
                self.m.const_array(et, &elems).map_err(|e| ir_err(&e))
            }
            TypeView::Record(rd) => self.null_record_const(rd, true),
            _ => Ok(self.m.const_int(t, -1)),
        }
    }

    /// clang's static `EmitNullConstant(CGM, record, asCompleteObject)`:
    /// the bases' and fields' null constants in the complete-object (or
    /// base-subobject) struct type, zero elsewhere (padding, bit-fields,
    /// the vptr). A union takes its first named member's.
    fn null_record_const(&mut self, rd: DeclId, complete: bool) -> R<Value> {
        let ast = self.ast;
        let first = ast.first(rd);
        let rec = self.tys.record(first).clone();
        let st = if complete {
            self.ir(&IrType::Record(first))
        } else {
            self.ir(&IrType::RecordBase(first))
        };
        let body = if complete { &rec.members } else { rec.base_members.as_ref().unwrap_or(&rec.members) };
        let tys: Vec<LTy> = body.iter().map(|m| self.ir(m)).collect();
        let mut elems: Vec<Option<Value>> = vec![None; tys.len()];
        let Some(def) = ast.definition(rd) else {
            return Ok(self.m.const_zero(st));
        };
        for b in ast.decls.bases(def, &ast.lists) {
            if b.is_virtual {
                continue;
            }
            let Some(bd) = ast.types.as_record(b.ty) else { continue };
            if let Some(&i) = rec.base_index.get(&ast.first(bd)) {
                let i = i as usize;
                elems[i] = Some(self.null_const_for_base(bd, tys[i])?);
            }
        }
        let fields: Vec<DeclId> =
            ast.decls.members(def).filter(|&m| ast.decls.get(m).kind() == DeclKind::Field).collect();
        for f in fields {
            let fq = ast.decls.get(f).ty().unwrap_or(QualType::INT);
            if !rec.bitfields.contains_key(&f) {
                if let Some(&i) = rec.field_index.get(&f) {
                    let i = i as usize;
                    elems[i] = Some(self.null_field_const(fq, tys[i])?);
                }
            }
            if rec.is_union && self.names_data_member(f) {
                break;
            }
        }
        if complete {
            let mut vbases: Vec<(DeclId, u32)> = rec.vbase_index.iter().map(|(&d, &i)| (d, i)).collect();
            vbases.sort_by_key(|&(_, i)| i);
            for (vb, i) in vbases {
                let i = i as usize;
                if elems[i].is_none() {
                    elems[i] = Some(self.null_const_for_base(vb, tys[i])?);
                }
            }
        }
        let vals: Vec<Value> =
            elems.iter().zip(&tys).map(|(v, &t)| v.unwrap_or_else(|| self.m.const_zero(t))).collect();
        self.m.const_struct(st, &vals).map_err(|e| ir_err(&e))
    }

    /// A field's null constant in its storage type `t` (a potentially
    /// overlapping member is stored as its class's base-subobject type).
    fn null_field_const(&mut self, fq: QualType, t: LTy) -> R<Value> {
        let v = self.null_const(fq)?;
        if self.m.const_type(v) == t {
            return Ok(v);
        }
        match self.ast.types.as_record(fq) {
            Some(rd) => self.null_const_for_base(rd, t),
            None => Ok(self.m.const_zero(t)),
        }
    }

    /// clang `EmitNullConstantForBase`.
    fn null_const_for_base(&mut self, base: DeclId, t: LTy) -> R<Value> {
        if self.tys.record(base).zero_init_as_base {
            return Ok(self.m.const_zero(t));
        }
        self.null_record_const(base, false)
    }

    /// The field has a name, or is an anonymous member whose class has a
    /// named data member (clang `findFirstNamedDataMember`): where the
    /// null constant of a union stops.
    fn names_data_member(&self, f: DeclId) -> bool {
        let ast = self.ast;
        let n = ast.decls.get(f);
        if !n.name().is_empty() {
            return true;
        }
        let Some(rd) = n.ty().and_then(|q| ast.types.as_record(q)) else { return false };
        let Some(def) = ast.definition(rd) else { return false };
        ast.decls
            .members(def)
            .filter(|&m| ast.decls.get(m).kind() == DeclKind::Field)
            .collect::<Vec<_>>()
            .into_iter()
            .any(|m| self.names_data_member(m))
    }

    /// A private constant global holding `init` (clang's unnamed
    /// `NullVariable`), aligned to `align`.
    fn null_global(&mut self, init: Value, align: u64) -> R<GlobalId> {
        let t = self.m.const_type(init);
        let g = self.m.add_global("", t);
        self.m.set_initializer(g, Some(init)).map_err(|e| ir_err(&e))?;
        let gv = self.m.global_mut(g);
        gv.is_constant = true;
        gv.linkage = Linkage::Private;
        gv.align = Some(align);
        Ok(g)
    }

    /// clang `CodeGenFunction::EmitNullInitialization` of `size` bytes of
    /// type `q` at `dest`: nothing for an empty class, a `memset` of 0
    /// when the null value is all-zero, else a `memcpy` from a private
    /// constant holding the null value.
    pub(crate) fn emit_null_init(&mut self, dest: Addr, q: QualType, size: u64) -> R<()> {
        let ast = self.ast;
        let mut base = q;
        while let Some(e) = self.array_elem(base) {
            base = e;
        }
        if base == q
            && ast.types.as_record(q).is_some_and(|rd| self.class_bits(rd).contains(DefinitionBits::EMPTY))
        {
            return Ok(());
        }
        if size == 0 {
            return Ok(());
        }
        if self.zero_init(q) {
            self.memset(dest, 0, size, false);
            return Ok(());
        }
        let init = self.null_const(q)?;
        let g = self.null_global(init, dest.align)?;
        self.memcpy(dest, V::C(Value::global(g)), dest.align, size, false);
        Ok(())
    }

    /// clang `EmitNullBaseClassInitialization`: zeroes the non-virtual
    /// part (`nv_size` bytes) of base subobject `rd` at `dest`, copying
    /// its base null constant when that is not all-zero.
    pub(crate) fn emit_null_base_init(&mut self, dest: Addr, rd: DeclId, nv_size: u64) -> R<()> {
        let z = V::C(self.m.const_int(LTy::I64, 0));
        let v = self.gep(LTy::I8, dest.v, vec![z], true);
        let at = Addr { v, ..dest };
        if self.tys.record(rd).zero_init_as_base {
            self.memset(at, 0, nv_size, false);
            return Ok(());
        }
        let first = self.ast.first(rd);
        let bt = self.ir(&IrType::RecordBase(first));
        let init = self.null_const_for_base(rd, bt)?;
        let nv_align = self.tys.layouter().record(rd).nv_align;
        let align = u64::from(nv_align).max(dest.align);
        let g = self.null_global(init, align)?;
        // The source's byte GEP by 0 folds to the global itself.
        self.memcpy(at, V::C(Value::global(g)), align, nv_size, false);
        Ok(())
    }
}
