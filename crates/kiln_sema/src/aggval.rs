//! Values of `constexpr` aggregates (spec/sema.md §8.3): folds the
//! semantic `InitListExpr` of an array or class object, element by
//! element through the [`ConstEvaluator`](crate::constfold::ConstEvaluator),
//! into the canonical [`ByteImage`] the AST stores.
//!
//! The M2 subset: arrays and non-union classes without bases or
//! bit-fields, whose leaves are integral, enumeration, `bool`, `float`,
//! `double`, null pointer or `nullptr_t` values. Anything else (unions,
//! bit-fields, string-literal array initializers, copies of other
//! aggregates, `long double`) is left unvalued without a diagnostic —
//! the Oven (WP-051) takes those over.

use kiln_ast::consts::{ByteImage, ByteRange, NullKind, Reloc, Scalar};
use kiln_ast::decl::{DeclKind, TagKind};
use kiln_ast::expr::ExprKind;
use kiln_ast::types::TypeView;
use kiln_ast::{ConstValueId, DeclId, ExprId, QualType};

use crate::Sema;
use crate::constfold::NotConstant;

/// Why an aggregate has no value.
#[derive(Debug)]
pub(crate) enum AggFail {
    /// A leaf is not a constant expression (diagnosed by the caller).
    NotConstant(NotConstant),
    /// The value is outside the M2 subset (left unvalued, no diagnostic).
    Unsupported,
}

impl From<NotConstant> for AggFail {
    fn from(nc: NotConstant) -> Self {
        AggFail::NotConstant(nc)
    }
}

/// An image under construction.
struct Image {
    bytes: Vec<u8>,
    /// Bytes that are part of a value (the rest is padding).
    covered: Vec<bool>,
    relocs: Vec<Reloc>,
}

/// Nesting bound (arrays of classes of arrays ...).
const MAX_DEPTH: u32 = 64;
/// Images larger than this are left to the Oven.
const MAX_BYTES: u64 = 1 << 20;

impl Sema<'_, '_> {
    /// True if objects of `ty` are valued by [`Sema::fold_aggregate`].
    pub(crate) fn is_aggregate_object(&self, ty: QualType) -> bool {
        matches!(
            self.ast.types.canonical_view(ty),
            TypeView::ConstantArray(_) | TypeView::Record(_)
        )
    }

    /// Folds the initializer `e` of an object of aggregate type `ty`.
    pub(crate) fn fold_aggregate(
        &mut self,
        ty: QualType,
        e: ExprId,
    ) -> Result<ConstValueId, AggFail> {
        let canon = self.ast.types.canonical(ty).without_quals();
        let size = self.object_size(canon)?;
        if size > MAX_BYTES {
            return Err(AggFail::Unsupported);
        }
        let size = usize::try_from(size).map_err(|_| AggFail::Unsupported)?;
        let mut img =
            Image { bytes: vec![0; size], covered: vec![false; size], relocs: Vec::new() };
        self.fold_object(canon, Some(e), 0, &mut img, 0)?;
        img.relocs.sort_by_key(|r| r.at);
        let mut padding: Vec<ByteRange> = Vec::new();
        for (i, &c) in img.covered.iter().enumerate() {
            if c {
                continue;
            }
            let i = u32::try_from(i).map_err(|_| AggFail::Unsupported)?;
            match padding.last_mut() {
                Some(r) if r.end() == u64::from(i) => r.len += 1,
                _ => padding.push(ByteRange { start: i, len: 1 }),
            }
        }
        let image = ByteImage {
            ty: canon,
            bytes: &img.bytes,
            relocs: &img.relocs,
            padding: &padding,
            unions: &[],
        };
        self.ast.consts.push_aggregate(image).map_err(|_| AggFail::Unsupported)
    }

    fn object_size(&self, ty: QualType) -> Result<u64, AggFail> {
        crate::target::size_align(&self.ast, ty, &self.abi)
            .map(|(s, _)| s)
            .ok_or(AggFail::Unsupported)
    }

    /// Writes the object of type `ty` at byte `at`, initialized by `e`
    /// (`None`: value-initialized).
    fn fold_object(
        &mut self,
        ty: QualType,
        e: Option<ExprId>,
        at: u64,
        img: &mut Image,
        depth: u32,
    ) -> Result<(), AggFail> {
        if depth > MAX_DEPTH {
            return Err(AggFail::Unsupported);
        }
        let e = e.filter(|&e| self.ast.exprs.get(e).kind() != ExprKind::ImplicitValueInitExpr);
        match self.ast.types.canonical_view(ty) {
            TypeView::ConstantArray(a) => {
                let stride = self.object_size(a.elem)?;
                let (inits, filler) = match e {
                    Some(e) => self.list_parts(e)?,
                    None => (Vec::new(), None),
                };
                if inits.len() as u64 > a.size {
                    return Err(AggFail::Unsupported);
                }
                for i in 0..a.size {
                    let init = usize::try_from(i).ok().and_then(|i| inits.get(i).copied());
                    let init = if init.is_some() { init } else { filler };
                    self.fold_object(a.elem, init, at + i * stride, img, depth + 1)?;
                }
                Ok(())
            }
            TypeView::Record(d) => {
                let fields = self.valued_fields(d)?;
                let def = self.ast.definition(d).ok_or(AggFail::Unsupported)?;
                let offsets = kiln_abi::ensure_layout(&mut self.ast, def, &self.abi).fields.clone();
                let inits = match e {
                    Some(e) => self.list_parts(e)?.0,
                    None => Vec::new(),
                };
                if inits.len() > fields.len() || offsets.len() != fields.len() {
                    return Err(AggFail::Unsupported);
                }
                for (i, (&f, &bits)) in fields.iter().zip(offsets.iter()).enumerate() {
                    let fty = self.ast.decls.get(f).ty().ok_or(AggFail::Unsupported)?;
                    self.fold_object(fty, inits.get(i).copied(), at + bits / 8, img, depth + 1)?;
                }
                Ok(())
            }
            _ => self.fold_scalar(ty, e, at, img),
        }
    }

    /// The explicit initializers and array filler of the list `e`.
    fn list_parts(&self, e: ExprId) -> Result<(Vec<ExprId>, Option<ExprId>), AggFail> {
        if self.ast.exprs.get(e).kind() != ExprKind::InitListExpr {
            return Err(AggFail::Unsupported);
        }
        let inits = self.ast.lists.iter(self.ast.exprs.init_list_inits(e)).collect();
        Ok((inits, self.ast.exprs.init_list(e).array_filler))
    }

    /// The fields of class `d` in declaration order, if the class is in
    /// the M2 subset (no union, bases or bit-fields).
    fn valued_fields(&self, d: DeclId) -> Result<Vec<DeclId>, AggFail> {
        let def = self.ast.definition(d).ok_or(AggFail::Unsupported)?;
        let rec = self.ast.decls.record(def).ok_or(AggFail::Unsupported)?;
        if rec.tag == TagKind::Union || !self.ast.lists.is_empty(rec.bases) {
            return Err(AggFail::Unsupported);
        }
        let mut out = Vec::new();
        for m in self.ast.members(def) {
            if self.ast.decls.get(m).kind() != DeclKind::Field {
                continue;
            }
            if self.ast.decls.field(m).is_some_and(|f| f.bit_width.is_some()) {
                return Err(AggFail::Unsupported);
            }
            out.push(m);
        }
        Ok(out)
    }

    /// Writes the scalar of type `ty` at byte `at`.
    fn fold_scalar(
        &mut self,
        ty: QualType,
        e: Option<ExprId>,
        at: u64,
        img: &mut Image,
    ) -> Result<(), AggFail> {
        let t = crate::conv::strip_cv(self, ty);
        let pointer_like = self.ast.types.is_pointer(t) || self.is_nullptr_type(t);
        let size = self.object_size(t)?;
        let start = usize::try_from(at).map_err(|_| AggFail::Unsupported)?;
        let len = usize::try_from(size).map_err(|_| AggFail::Unsupported)?;
        let end = start.checked_add(len).filter(|&end| end <= img.bytes.len());
        let end = end.ok_or(AggFail::Unsupported)?;
        let id = match e {
            Some(e) => {
                let v = self.evaluate(e)?;
                self.const_value(t, v).ok_or(AggFail::Unsupported)?
            }
            None if pointer_like => self
                .ast
                .consts
                .push_scalar(Scalar::Null { ty: t, kind: NullKind::Pointer })
                .map_err(|_| AggFail::Unsupported)?,
            None => {
                if !(self.ast.types.is_arithmetic(t) || self.ast.types.as_enum(t).is_some()) {
                    return Err(AggFail::Unsupported);
                }
                img.covered[start..end].fill(true);
                return Ok(());
            }
        };
        let raw: u128 = match self.ast.consts.scalar(id).ok_or(AggFail::Unsupported)? {
            Scalar::Int { signed, bits, .. } => {
                #[allow(clippy::cast_sign_loss, reason = "two's-complement image bytes")]
                let r = if signed { bits.to_i128().map(|v| v as u128) } else { bits.to_u128() };
                r.ok_or(AggFail::Unsupported)?
            }
            Scalar::Bool { value, .. } => u128::from(value),
            Scalar::Float { kind, bits, .. } => {
                if kind.value_bytes() as usize != len {
                    return Err(AggFail::Unsupported);
                }
                bits
            }
            Scalar::Null { kind: NullKind::Pointer, .. } => {
                let at = u32::try_from(at).map_err(|_| AggFail::Unsupported)?;
                img.relocs.push(Reloc { at, value: id });
                img.covered[start..end].fill(true);
                return Ok(());
            }
            _ => return Err(AggFail::Unsupported),
        };
        if len > 16 {
            return Err(AggFail::Unsupported);
        }
        img.bytes[start..end].copy_from_slice(&raw.to_le_bytes()[..len]);
        img.covered[start..end].fill(true);
        Ok(())
    }
}
