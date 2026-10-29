//! Builtin functions (spec/sema.md §5.4): clang's `LazilyCreateBuiltin`.
//!
//! A builtin function is declared the first time an unqualified lookup of
//! its name finds nothing: an implicit `extern "C"` `Function` decl in the
//! translation unit (inside an implicit `LinkageSpec`, as clang dumps it)
//! carrying its builtin id in the `builtin_id` cold data, an implicit
//! `BuiltinAttr` and, for the `noexcept` ones, an implicit `NoThrowAttr`.
//! Its name has the placeholder type `<builtin fn type>`; a call converts
//! it with `BuiltinFnToFnPtr` ([`Sema::act_on_call`]), and any other use is
//! an error.

use crate::lookup::LookupKind;
use kiln_ast::attr::{AttrKind, AttrNode, AttrSyntax};
use kiln_ast::consts::{PtrBase, Scalar};
use kiln_ast::decl::{DeclFlags, DeclKind, FnSpec, FnStorage, LinkageLang, builtin};
use kiln_ast::expr::{CastKind, ExprFlags, ExprKind, ValueCategory};
use kiln_ast::expr::{SourceLocKind, SourceLocValues};
use kiln_ast::names::{DeclName, OverloadedOperator};
use kiln_ast::types::{BuiltinType, ExcSpec, FuncInfo, Quals};
use kiln_ast::types::{Nns, TagKeyword};
use kiln_ast::{DeclHead, DeclId, ExprId, QualType};
use kiln_diag::DiagId;
use kiln_source::Loc;
use kiln_source::symbol::Symbol;

use crate::Sema;
use crate::conv::{self, ConvCtx};

impl Sema<'_, '_> {
    /// Declares the builtin function `name` (first used at `loc`) if it
    /// is one kiln knows, from its [`builtin::Info`] row: the type its
    /// `Builtins.td` signature spells (`noexcept` for the `NoThrow` ones,
    /// clang's `GetBuiltinType`), one implicit parameter per parameter
    /// type, and the implicit attributes in clang's order (`BuiltinAttr`,
    /// then `NoThrowAttr`, `ConstAttr` / `PureAttr`).
    pub(crate) fn lazily_create_builtin(&mut self, name: Symbol, loc: Loc) -> Option<DeclId> {
        let id = builtin::by_name(self.ast.symbols().as_str(name))?;
        let info = *builtin::info(id)?;
        let (ret, params, variadic) = self.builtin_signature(info.sig);
        let has = |a: u8| info.attrs & a != 0;
        let fi =
            FuncInfo::new().with_variadic(variadic).with_noreturn(has(builtin::attr::NORETURN));
        let exc = if has(builtin::attr::NOTHROW) { ExcSpec::BasicNoexcept } else { ExcSpec::None };
        let ty = self.ast.types.function(ret, &params, fi, exc);
        let tu = self.ast.tu;
        let lh = DeclHead::new(tu, DeclName::EMPTY, loc).flags(DeclFlags::IMPLICIT);
        let ls = self.builder().linkage_spec_head(lh, LinkageLang::C);
        let h = DeclHead::new(ls, DeclName::ident(name), loc).flags(DeclFlags::IMPLICIT);
        let spec = FnSpec::new().with_storage(FnStorage::Extern);
        let f = self.builder().function(h, DeclKind::Function, ty, spec, None);
        let _ = self.ast.decls.cold.builtin_id.insert(f, id);
        let implicit =
            |kind, loc, a| AttrNode { kind, syntax: AttrSyntax::Gnu, flags: 1, loc, a, b: 0 };
        let mut pds = Vec::with_capacity(params.len());
        for (i, &pty) in params.iter().enumerate() {
            let idx = u16::try_from(i).unwrap_or(u16::MAX);
            let ph = DeclHead::new(f, DeclName::EMPTY, Loc::INVALID).flags(DeclFlags::IMPLICIT);
            let p = self.builder().param_head(ph, pty, idx, 0);
            if id == builtin::ADDRESSOF {
                // clang: an implicit `[[clang::lifetimebound]]` parameter.
                let lb = self.ast.attrs.push(implicit(AttrKind::LIFETIMEBOUND, loc, 0));
                let _ = self.builder().attach_attrs(p, &[lb]);
            }
            pds.push(p);
        }
        if !pds.is_empty() {
            let _ = self.builder().set_params(f, &pds);
        }
        let mut attrs = vec![self.ast.attrs.push(implicit(AttrKind::BUILTIN, Loc::INVALID, id))];
        for (bit, kind) in [
            (builtin::attr::NOTHROW, AttrKind::NOTHROW),
            (builtin::attr::CONST, AttrKind::CONST),
            (builtin::attr::PURE, AttrKind::PURE),
        ] {
            if has(bit) {
                attrs.push(self.ast.attrs.push(implicit(kind, loc, 0)));
            }
        }
        let _ = self.builder().attach_attrs(f, &attrs);
        Some(f)
    }

    /// The return type, parameter types and variadic bit a `Builtins.td`
    /// signature spells (clang's `DecodeTypeFromStr`; see
    /// [`builtin::Info::sig`]).
    fn builtin_signature(&mut self, sig: &str) -> (QualType, Vec<QualType>, bool) {
        let mut s = sig.as_bytes();
        let mut tys = Vec::new();
        let mut variadic = false;
        while let Some(&c) = s.first() {
            if c == b'.' {
                variadic = true;
                break;
            }
            let (t, rest) = self.builtin_sig_type(s);
            tys.push(t);
            s = rest;
        }
        let ret = if tys.is_empty() { QualType::VOID } else { tys.remove(0) };
        (ret, tys, variadic)
    }

    /// One type of a `Builtins.td` signature and the rest of the string.
    fn builtin_sig_type<'s>(&mut self, mut s: &'s [u8]) -> (QualType, &'s [u8]) {
        let (mut unsigned, mut longs) = (false, 0);
        while let Some(&c) = s.first() {
            match c {
                b'U' => unsigned = true,
                b'L' => longs += 1,
                _ => break,
            }
            s = &s[1..];
        }
        let base = s.first().copied().unwrap_or(b'v');
        s = s.get(1..).unwrap_or_default();
        let mut t = match (base, unsigned, longs) {
            (b'b', ..) => QualType::BOOL,
            (b'c', false, _) => QualType::CHAR,
            (b'c', true, _) => QualType::UCHAR,
            (b'w', ..) => QualType::WCHAR,
            (b's', false, _) => QualType::SHORT,
            (b's', true, _) => QualType::USHORT,
            (b'i', false, 0) => QualType::INT,
            (b'i', true, 0) => QualType::UINT,
            (b'i', false, 1) => QualType::LONG,
            (b'i', true, 1) => QualType::ULONG,
            (b'i', false, _) => QualType::LONGLONG,
            (b'i', true, _) => QualType::ULONGLONG,
            (b'f', ..) => QualType::FLOAT,
            (b'd', _, 0) => QualType::DOUBLE,
            (b'd', ..) => QualType::LONGDOUBLE,
            (b'z', ..) => self.size_type(),
            _ => QualType::VOID,
        };
        while let Some(&c) = s.first() {
            t = match c {
                b'C' => self.ast.types.qualify(t, Quals::CONST),
                b'*' => self.ast.types.pointer_to(t),
                b'&' => self.ast.types.lvalue_ref_to(t),
                _ => break,
            };
            s = &s[1..];
        }
        (t, s)
    }

    /// A call of a builtin with custom type-checking (clang's
    /// `CustomTypeChecking` builtins, checked in `CheckBuiltinFunctionCall`):
    /// the arguments are not converted to parameter types; each builtin
    /// converts and checks them and decides the call's type. Returns the
    /// call, or a recovery expression after a diagnostic.
    #[allow(clippy::too_many_arguments, reason = "one per part of the call")]
    pub(crate) fn builtin_custom_call(
        &mut self,
        callee: ExprId,
        b: DeclId,
        id: u32,
        args: &[ExprId],
        braced: bool,
        loc: Loc,
        rparen: Loc,
    ) -> ExprId {
        let mut kids = vec![callee];
        kids.extend_from_slice(args);
        if id == builtin::ADDRESSOF {
            return match self.builtin_addressof_call(callee, b, args, braced, loc, rparen) {
                Some(e) => e,
                None => self.recover_err(&kids, loc),
            };
        }
        if braced {
            self.unsupported("a braced argument to a builtin function", loc);
            return self.recover_err(&kids, loc);
        }
        let r = match id {
            builtin::CLZG | builtin::CTZG => self.builtin_count_zeros_generic(args, rparen),
            builtin::POPCOUNTG => self.builtin_popcountg(args, rparen),
            builtin::ISFINITE
            | builtin::ISINF
            | builtin::ISNAN
            | builtin::ISNORMAL
            | builtin::SIGNBIT => self.builtin_fp_classification(args, rparen),
            builtin::ISGREATER
            | builtin::ISGREATEREQUAL
            | builtin::ISLESS
            | builtin::ISLESSEQUAL
            | builtin::ISLESSGREATER
            | builtin::ISUNORDERED => self.builtin_unordered_compare(args, rparen),
            builtin::LAUNDER => self.builtin_launder(callee, args, rparen),
            builtin::OPERATOR_NEW | builtin::OPERATOR_DELETE => {
                return self.builtin_operator_new_delete(callee, b, id, args, loc, rparen);
            }
            _ => {
                self.unsupported("this builtin function", loc);
                None
            }
        };
        let Some((args, ty)) = r else { return self.recover_err(&kids, loc) };
        let f = self.builtin_fn_to_ptr(callee, b);
        self.builder().call(f, &args, ty, ValueCategory::PRValue, loc, rparen, ExprFlags::NONE)
    }

    /// clang's `checkArgCount`: exactly `n` arguments, else `too few`
    /// at the `)` or `too many` at the first extra argument.
    fn builtin_arg_count(&mut self, args: &[ExprId], n: usize, rparen: Loc) -> bool {
        if args.len() == n {
            return true;
        }
        let (id, at) = if args.len() < n {
            (DiagId::sema_call_too_few_args, rparen)
        } else {
            (DiagId::sema_call_too_many_args, self.begin_loc(args[n]))
        };
        let msg = self.diag(id, at).arg(n).arg(args.len());
        self.emit(msg);
        false
    }

    /// clang's `isUnsignedIntegerType`: `bool`, an unsigned (or
    /// unsigned-character) integer type, an unsigned `_BitInt`, or a
    /// complete unscoped enumeration with an unsigned underlying type.
    fn is_unsigned_integer_type(&self, q: QualType) -> bool {
        if self.ast.types.as_enum(q).is_some() {
            return conv::rank::is_unscoped_enum(self, q)
                && conv::rank::int_width(self, q).is_some()
                && !conv::rank::int_signed(self, q);
        }
        self.ast.types.is_integral(q) && !conv::rank::int_signed(self, q)
    }

    /// `__builtin_clzg` / `__builtin_ctzg` (clang's
    /// `BuiltinCountZeroBitsGeneric`): one unsigned integer argument and
    /// an optional `int` fallback for a zero value.
    fn builtin_count_zeros_generic(
        &mut self,
        args: &[ExprId],
        rparen: Loc,
    ) -> Option<(Vec<ExprId>, QualType)> {
        match args.len() {
            0 => {
                let msg = self.diag(DiagId::sema_call_too_few_args, rparen).arg(1).arg(0);
                self.emit(msg);
                return None;
            }
            1 | 2 => {}
            n => {
                let msg = self.diag(DiagId::sema_call_too_many_args_at_most, rparen).arg(2).arg(n);
                self.emit(msg);
                return None;
            }
        }
        let a = conv::lvalue_to_rvalue(self, args[0]);
        let t = self.ety(a);
        if !self.is_unsigned_integer_type(t) {
            let msg = self
                .diag(DiagId::sema_builtin_invalid_arg_type, self.begin_loc(a))
                .arg(0)
                .arg(0)
                .arg(self.ty_arg(t));
            self.emit(msg);
            return None;
        }
        let mut out = vec![a];
        if let Some(&fb) = args.get(1) {
            let fb = conv::usual_unary_conversions(self, fb);
            let ft = self.ety(fb);
            if !self.ast.types.same_unqualified(ft, QualType::INT) {
                let msg = self
                    .diag(DiagId::sema_builtin_invalid_arg_type, self.begin_loc(fb))
                    .arg(1)
                    .arg(1)
                    .arg(self.ty_arg(ft));
                self.emit(msg);
                return None;
            }
            out.push(fb);
        }
        Some((out, QualType::INT))
    }

    /// `__builtin_popcountg` (clang's `BuiltinPopcountg`): one unsigned
    /// integer argument.
    fn builtin_popcountg(
        &mut self,
        args: &[ExprId],
        rparen: Loc,
    ) -> Option<(Vec<ExprId>, QualType)> {
        if !self.builtin_arg_count(args, 1, rparen) {
            return None;
        }
        let a = conv::lvalue_to_rvalue(self, args[0]);
        let t = self.ety(a);
        if !self.is_unsigned_integer_type(t) {
            let msg = self
                .diag(DiagId::sema_builtin_invalid_arg_type, self.begin_loc(a))
                .arg(0)
                .arg(0)
                .arg(self.ty_arg(t));
            self.emit(msg);
            return None;
        }
        Some((vec![a], QualType::INT))
    }

    /// `__builtin_isfinite` & co. (clang's `BuiltinFPClassification`;
    /// C23 7.12.3): one real floating argument.
    fn builtin_fp_classification(
        &mut self,
        args: &[ExprId],
        rparen: Loc,
    ) -> Option<(Vec<ExprId>, QualType)> {
        if !self.builtin_arg_count(args, 1, rparen) {
            return None;
        }
        let a = conv::decay(self, args[0]);
        let t = self.ety(a);
        if !self.is_real_floating(t) {
            let msg =
                self.diag(DiagId::sema_builtin_fp_class_arg, self.begin_loc(a)).arg(self.ty_arg(t));
            self.emit(msg);
            return None;
        }
        Some((vec![a], QualType::INT))
    }

    /// `__builtin_isgreater` & co. (clang's `BuiltinUnorderedCompare`;
    /// C23 7.12.17): the usual arithmetic conversions to a real floating
    /// common type.
    fn builtin_unordered_compare(
        &mut self,
        args: &[ExprId],
        rparen: Loc,
    ) -> Option<(Vec<ExprId>, QualType)> {
        if !self.builtin_arg_count(args, 2, rparen) {
            return None;
        }
        let l = conv::usual_unary_conversions(self, args[0]);
        let r = conv::usual_unary_conversions(self, args[1]);
        let (lt, rt) = (self.ety(l), self.ety(r));
        let arith = |s: &Self, q| conv::rank::is_arithmetic_or_unscoped_enum(s, q);
        let (l, r, common) = if arith(self, lt) && arith(self, rt) {
            match conv::usual_arithmetic_conversions_kind(
                self,
                l,
                r,
                conv::ArithConvKind::Comparison,
            ) {
                Ok(x) => x,
                Err(e) => {
                    let msg = e.diag(self, l);
                    self.emit(msg);
                    return None;
                }
            }
        } else {
            (l, r, QualType::ERROR)
        };
        if !self.is_real_floating(common) {
            let msg = self
                .diag(DiagId::sema_builtin_ordered_compare, self.begin_loc(l))
                .arg(self.ty_arg(self.ety(l)))
                .arg(self.ty_arg(self.ety(r)));
            self.emit(msg);
            return None;
        }
        Some((vec![l, r], QualType::INT))
    }

    /// A real (not complex) floating type.
    fn is_real_floating(&self, q: QualType) -> bool {
        self.ast.types.builtin(q).is_some_and(BuiltinType::is_floating)
    }

    /// `__builtin_launder` ([ptr.launder]; clang's `BuiltinLaunder`): one
    /// argument of pointer-to-complete-object type (an array or function
    /// argument decays), copy-initialized to that type, which is the
    /// call's.
    fn builtin_launder(
        &mut self,
        callee: ExprId,
        args: &[ExprId],
        rparen: Loc,
    ) -> Option<(Vec<ExprId>, QualType)> {
        if !self.builtin_arg_count(args, 1, rparen) {
            return None;
        }
        let at = self.begin_loc(callee);
        let a = args[0];
        let t = self.ety(a);
        let pt = if self.ast.types.is_array(t) {
            conv::array_decayed(self, t)
        } else if self.ast.types.is_function(t) {
            self.ast.types.pointer_to(t)
        } else {
            t
        };
        let pointee = self.ast.types.pointee(pt).filter(|_| self.ast.types.is_pointer(pt));
        let bad = match pointee {
            None => Some(0),
            Some(p) if self.ast.types.is_function(p) => Some(1),
            Some(p) if self.ast.types.is_void(p) => Some(2),
            Some(_) => None,
        };
        if let Some(which) = bad {
            let msg = self.diag(DiagId::sema_builtin_launder_invalid_arg, at).arg(which);
            self.emit(msg);
            return None;
        }
        let pointee = pointee.unwrap_or(QualType::ERROR);
        if !self.require_complete(pointee, at) {
            let msg =
                self.diag(DiagId::sema_incomplete_type_required, at).arg(self.ty_arg(pointee));
            self.emit(self.incomplete_note(msg, pointee));
            return None;
        }
        let pt = conv::strip_cv(self, pt);
        match conv::implicit_convert(self, a, pt, ConvCtx::Argument) {
            Ok(x) => Some((vec![x], pt)),
            Err(err) => {
                let msg = err.diag(self, a);
                self.emit(msg);
                None
            }
        }
    }

    /// `__builtin_operator_new` / `__builtin_operator_delete` (clang's
    /// `BuiltinOperatorNewDeleteOverloaded`): overload resolution among
    /// the global `operator new` / `operator delete` functions
    /// ([basic.stc.dynamic.allocation], [basic.stc.dynamic.deallocation])
    /// that must select a replaceable one ([new.delete]); the call has its
    /// return type, the arguments are copy-initialized to its parameters,
    /// and the callee's `BuiltinFnToFnPtr` cast has its function type.
    fn builtin_operator_new_delete(
        &mut self,
        callee: ExprId,
        b: DeclId,
        id: u32,
        args: &[ExprId],
        loc: Loc,
        rparen: Loc,
    ) -> ExprId {
        let mut kids = vec![callee];
        kids.extend_from_slice(args);
        let delete = id == builtin::OPERATOR_DELETE;
        let op = if delete { OverloadedOperator::Delete } else { OverloadedOperator::New };
        self.declare_global_new_delete();
        let at = self.begin_loc(callee);
        let tu = self.ast.tu;
        let cands = self.ast.lookup_local(tu, DeclName::operator(op)).to_vec();
        let mut best: Option<(DeclId, usize)> = None;
        for &f in &cands {
            if self.ast.decls.get(f).kind() != DeclKind::Function {
                continue;
            }
            let Some((ps, variadic)) = self.fn_params(f) else { continue };
            if ps.len() > args.len() || (ps.len() < args.len() && !variadic) {
                continue;
            }
            let mut exact = 0;
            let ok = args.iter().zip(&ps).all(|(&a, &p)| match self.placement_arg_match(a, p) {
                Some(e) => {
                    exact += usize::from(e);
                    true
                }
                None => false,
            });
            if ok && best.is_none_or(|(_, e)| exact > e) {
                best = Some((f, exact));
            }
        }
        let name = format!("operator {}", if delete { "delete" } else { "new" });
        let Some((f, _)) = best else {
            let msg = self.diag(DiagId::sema_ovl_no_viable_function_in_call, at).arg(&name);
            let msg = self.builtin_new_delete_notes(msg, &cands, args);
            self.emit(msg);
            return self.recover_err(&kids, loc);
        };
        if !self.is_replaceable_global_alloc(f, delete) {
            let msg =
                self.diag(DiagId::sema_builtin_new_delete_not_usual, at).arg(usize::from(delete));
            let fl = self.ast.decls.get(f).loc();
            let note = self
                .diag(DiagId::note_builtin_non_usual_declared_here, fl)
                .arg(format!("'{name}'"));
            self.emit(msg.with_note(note));
            return self.recover_err(&kids, loc);
        }
        let Some((ps, _)) = self.fn_params(f) else { return self.recover_err(&kids, loc) };
        let mut out = Vec::with_capacity(args.len());
        for (&a, &p) in args.iter().zip(&ps) {
            let p = conv::strip_cv(self, p);
            match conv::implicit_convert(self, a, p, ConvCtx::Argument) {
                Ok(x) => out.push(x),
                Err(err) => {
                    let msg = err.diag(self, a);
                    self.emit(msg);
                    return self.recover_err(&kids, loc);
                }
            }
        }
        self.mark_used(f);
        let fty = self.ast.decls.get(f).ty().unwrap_or(QualType::ERROR);
        let _ = b;
        let cal = conv::raw_cast(
            self,
            CastKind::BuiltinFnToFnPtr,
            callee,
            fty,
            ValueCategory::PRValue,
            None,
        );
        let ret = self.ast.types.proto(fty).map_or(QualType::ERROR, |p| p.ret());
        let ret = conv::strip_cv(self, ret);
        self.builder().call(cal, &out, ret, ValueCategory::PRValue, loc, rparen, ExprFlags::NONE)
    }

    /// clang's `isReplaceableGlobalAllocationFunction` for a global
    /// `operator new` (`delete`) `f`: `(std::size_t` (`void*`), then
    /// optionally a `std::size_t` size (deallocation only), a
    /// `std::align_val_t` and a `const std::nothrow_t&`, in that order.
    fn is_replaceable_global_alloc(&self, f: DeclId, delete: bool) -> bool {
        let Some((ps, variadic)) = self.fn_params(f) else { return false };
        if variadic || ps.is_empty() {
            return false;
        }
        let t = &self.ast.types;
        let size_t = self.size_type();
        let first_ok = if delete {
            t.is_pointer(ps[0]) && t.pointee(ps[0]).is_some_and(|p| t.same_type(p, QualType::VOID))
        } else {
            t.same_type(ps[0], size_t)
        };
        if !first_ok {
            return false;
        }
        let mut rest = &ps[1..];
        if delete && rest.first().is_some_and(|&p| t.same_type(p, size_t)) {
            rest = &rest[1..];
        }
        if let (Some(&p), Some(al)) = (rest.first(), self.global_ops.align_val_t)
            && t.same_type(p, al)
        {
            rest = &rest[1..];
        }
        if let Some(&p) = rest.first()
            && self.is_nothrow_t_ref(p)
        {
            rest = &rest[1..];
        }
        rest.is_empty()
    }

    /// `const std::nothrow_t &`.
    fn is_nothrow_t_ref(&self, p: QualType) -> bool {
        let t = &self.ast.types;
        if !t.is_reference(p) {
            return false;
        }
        let Some(r) = t.pointee(p).and_then(|x| t.as_record(x)) else { return false };
        let d = self.ast.decls.get(r);
        d.name().as_ident().is_some_and(|n| self.ast.symbols().as_str(n) == "nothrow_t")
    }

    /// clang's candidate notes for a failed builtin `operator new` /
    /// `operator delete` resolution (the bad conversions first, then the
    /// arity mismatches, closest first).
    fn builtin_new_delete_notes(
        &mut self,
        mut msg: kiln_diag::Diag,
        cands: &[DeclId],
        args: &[ExprId],
    ) -> kiln_diag::Diag {
        let have = args.len();
        let mut bad: Vec<(bool, Loc, String)> = Vec::new();
        let mut arity: Vec<(usize, bool, Loc, String)> = Vec::new();
        for &f in cands {
            let Some((ps, variadic)) = self.fn_params(f) else { continue };
            let implicit = self.ast.decls.get(f).flags().contains(DeclFlags::IMPLICIT);
            let fl = if implicit { Loc::INVALID } else { self.ast.decls.get(f).loc() };
            let n = ps.len();
            if have < n || (have > n && !variadic) {
                let detail = self.arity_detail(f, n, n, have, variadic);
                arity.push((n.abs_diff(have), implicit, fl, detail));
                continue;
            }
            for (i, (&a, &p)) in args.iter().zip(&ps).enumerate() {
                if self.placement_arg_match(a, p).is_some() {
                    continue;
                }
                let detail = format!(
                    "no known conversion from {} to {} for {} argument",
                    self.ty_arg(self.ety(a)),
                    self.ty_arg(p),
                    crate::expr::ordinal(i + 1)
                );
                bad.push((implicit, fl, detail));
                break;
            }
        }
        bad.sort_by_key(|b| b.0);
        arity.sort_by_key(|a| (a.0, !a.1));
        let notes: Vec<(Loc, String)> = bad
            .into_iter()
            .map(|(_, l, d)| (l, d))
            .chain(arity.into_iter().map(|(_, _, l, d)| (l, d)))
            .collect();
        for (l, d) in notes {
            let note = self.diag(DiagId::note_ovl_candidate_not_viable, l).arg(d);
            msg = msg.with_note(note);
        }
        msg
    }

    /// A call of `__builtin_addressof` (clang's custom type-checking,
    /// `SemaBuiltinAddressof`): exactly one argument, which must be an
    /// lvalue that is not a bit-field ([specialized.addressof]); the type
    /// is a pointer to the argument's type and the argument is not
    /// converted. `None` after a diagnostic.
    pub(crate) fn builtin_addressof_call(
        &mut self,
        callee: ExprId,
        b: DeclId,
        args: &[ExprId],
        braced: bool,
        loc: Loc,
        rparen: Loc,
    ) -> Option<ExprId> {
        let at = self.begin_loc(callee);
        match args {
            [] => {
                let msg = self.diag(DiagId::sema_call_too_few_args, rparen).arg(1).arg(0);
                self.emit(msg);
                return None;
            }
            [_] => {}
            [_, extra, ..] => {
                let msg = self
                    .diag(DiagId::sema_call_too_many_args, self.begin_loc(*extra))
                    .arg(1)
                    .arg(args.len());
                self.emit(msg);
                return None;
            }
        }
        if braced {
            self.unsupported("a braced argument to __builtin_addressof", loc);
            return None;
        }
        let ty = self.check_addressable(args[0], at)?;
        let f = self.builtin_fn_to_ptr(callee, b);
        let ty = self.ast.types.pointer_to(ty);
        Some(self.builder().call(
            f,
            &[args[0]],
            ty,
            ValueCategory::PRValue,
            loc,
            rparen,
            ExprFlags::NONE,
        ))
    }

    /// Whether unqualified lookup of `name` would declare a builtin
    /// function (clang's `LookupBuiltin`): the parser then treats a call of
    /// it in a template as naming a function, not as an ADL-only name.
    pub fn is_builtin_name(&self, name: Symbol) -> bool {
        builtin::by_name(self.ast.symbols().as_str(name))
            .is_some_and(|id| builtin::info(id).is_some())
    }

    /// The builtin function `callee` (through parentheses) names, if it
    /// names one.
    pub(crate) fn builtin_callee(&self, callee: ExprId) -> Option<DeclId> {
        let x = self.ignore_parens(callee);
        let n = self.ast.exprs.get(x);
        if n.kind() != ExprKind::DeclRefExpr {
            return None;
        }
        let d = self.ast.exprs.decl(x);
        self.ast.decls.builtin_id(d).map(|_| d)
    }

    /// The callee of a call to the builtin `d`: its name converted to a
    /// pointer to its function type (clang's `BuiltinFnToFnPtr`).
    pub(crate) fn builtin_fn_to_ptr(&mut self, callee: ExprId, d: DeclId) -> ExprId {
        let fty = self.ast.decls.get(d).ty().unwrap_or(QualType::ERROR);
        let p = self.ast.types.pointer_to(fty);
        conv::raw_cast(self, CastKind::BuiltinFnToFnPtr, callee, p, ValueCategory::PRValue, None)
    }

    /// Whether `e` is a builtin function's name used other than as a
    /// callee; reports `builtin functions must be directly called`.
    pub(crate) fn check_builtin_fn_use(&mut self, e: ExprId) -> bool {
        if self.ast.exprs.get(e).ty() == QualType::BOUND_MEMBER {
            // [expr.ref]/7.3.2: a bound member function can only be called.
            self.diagnose_bound_member(e);
            return true;
        }
        if self.ast.exprs.get(e).ty() != QualType::BUILTIN_FN {
            return false;
        }
        let msg = self.diag(DiagId::sema_builtin_fn_use, self.eloc(e));
        self.emit(msg);
        true
    }
}

impl Sema<'_, '_> {
    /// `__builtin_bit_cast(ty, e)` at `loc` ([bit.cast]; clang
    /// `BuildBuiltinBitCastExpr` and `CastOperation::CheckBuiltinBitCast`):
    /// both types complete, a prvalue operand materialized, equal sizes,
    /// and both types trivially copyable. The result is a prvalue of `ty`
    /// (cv-unqualified for a non-class type) with cast kind
    /// `LValueToRValueBitCast`. Evaluating it is an Oven stub
    /// (spec/sema-templates.md §9.5).
    pub fn act_on_builtin_bit_cast(
        &mut self,
        ty: QualType,
        e: ExprId,
        loc: Loc,
        rparen: Loc,
    ) -> ExprId {
        use kiln_ast::expr::{ExprFlags, StorageDuration, ValueCategory};
        let _ = rparen;
        if self.is_err_expr(e) || self.is_error_type(ty) {
            return self.recover(self.non_reference(ty), &[e], loc);
        }
        if self.ast.types.is_dependent(ty) || self.expr_is_type_dependent(e) {
            let to = self.non_reference(ty);
            return self.builder().cast(
                ExprKind::BuiltinBitCastExpr,
                CastKind::Dependent,
                e,
                to,
                ValueCategory::PRValue,
                None,
                loc,
                ExprFlags::NONE,
            );
        }
        let dest = self.non_reference(ty);
        if !self.require_complete(dest, loc) {
            let msg = self.diag(DiagId::sema_cast_to_incomplete, loc).arg(self.ty_arg(dest));
            self.emit(self.incomplete_note(msg, dest));
            return self.recover_err(&[e], loc);
        }
        let src = self.ety(e);
        if !self.require_complete(src, loc) {
            let msg = self.diag(DiagId::sema_incomplete_type_required, loc).arg(self.ty_arg(src));
            self.emit(self.incomplete_note(msg, src));
            return self.recover_err(&[e], loc);
        }
        let mut sub = e;
        if self.ast.exprs.get(e).value_category() == ValueCategory::PRValue {
            // clang's `CreateMaterializeTemporaryExpr` (its full-expression
            // needs cleanups, without side effects).
            sub = self.builder().materialize_temporary(
                e,
                src,
                false,
                StorageDuration::FullExpression,
                None,
            );
            self.cleanup.needs = true;
        }
        // A reference destination has the size of a pointer (clang's
        // `getTypeSizeInChars` of a reference type).
        let size_of = |s: &mut Self, t: QualType| {
            let t = if s.ast.types.is_reference(t) { s.ast.types.pointer_to(t) } else { t };
            crate::target::size_align(&s.ast, t, &s.abi).map_or(0, |x| x.0)
        };
        let (ss, ds) = (size_of(self, src), size_of(self, ty));
        if ss != ds {
            let msg = self.diag(DiagId::sema_bit_cast_size_mismatch, loc).arg(ss).arg(ds);
            self.emit(msg);
            return self.recover_err(&[sub], loc);
        }
        for (t, which) in [(ty, "1"), (src, "0")] {
            if self.ast.types.is_reference(t) || !self.is_trivially_copyable_type(t) {
                let msg = self.diag(DiagId::sema_bit_cast_not_trivially_copyable, loc).arg(which);
                self.emit(msg);
                return self.recover_err(&[sub], loc);
            }
        }
        let to = if self.ast.types.as_record(dest).is_some() || self.ast.types.is_array(dest) {
            dest
        } else {
            conv::strip_cv(self, dest)
        };
        self.builder().cast(
            ExprKind::BuiltinBitCastExpr,
            CastKind::LValueToRValueBitCast,
            sub,
            to,
            ValueCategory::PRValue,
            None,
            loc,
            ExprFlags::NONE,
        )
    }
}

// ---- source-location builtins -------------------------------------------

/// Per-TU state of the source-location builtins (clang
/// `StdSourceLocationImplDecl` and `UniqueStringLiterals`).
#[derive(Debug, Default)]
pub(crate) struct SrcLocCache {
    /// `std::source_location::__impl` once found and checked.
    impl_decl: Option<DeclId>,
    /// The string literal object every source-location string with this
    /// content points into: one `StringLiteral` expression per content
    /// (clang `getPredefinedStringLiteralFromCache`).
    strings: kiln_source::fx::FxHashMap<kiln_ast::StringLitId, ExprId>,
    /// Which builtins a parameter's default argument evaluates,
    /// transitively through the default arguments of the calls in it
    /// (bit `1 << kind`; computed once per parameter).
    param_kinds: kiln_source::fx::FxHashMap<DeclId, u8>,
}

/// The bit of `SourceLocKind::SourceLocStruct` in a kinds mask.
const SRC_LOC_STRUCT_BIT: u8 = 1 << SourceLocKind::SourceLocStruct as u8;

impl Sema<'_, '_> {
    /// `__builtin_FILE()`, `__builtin_FILE_NAME()`, `__builtin_FUNCTION()`,
    /// `__builtin_LINE()`, `__builtin_COLUMN()` and
    /// `__builtin_source_location()` (clang `ActOnSourceLocExpr`;
    /// [support.srcloc.cons]): a prvalue of type `const char *`,
    /// `unsigned int` or `const std::source_location::__impl *`. The values
    /// at the expression itself are computed now (§9.5); a use of a default
    /// argument holding one gets the values of the call
    /// ([`Self::note_default_arg_use`]).
    pub fn act_on_source_loc_expr(&mut self, kind: SourceLocKind, loc: Loc, rparen: Loc) -> ExprId {
        let ty = match kind {
            SourceLocKind::File | SourceLocKind::FileName | SourceLocKind::Function => {
                self.const_char_ptr(QualType::CHAR)
            }
            SourceLocKind::Line | SourceLocKind::Column => QualType::UINT,
            SourceLocKind::SourceLocStruct => match self.std_source_location_impl(loc) {
                Some(t) => {
                    let c = self.ast.types.qualify(t, Quals::CONST);
                    self.ast.types.pointer_to(c)
                }
                None => return self.recover_err(&[], loc),
            },
        };
        let ctx = self.ctx;
        let e = self.builder().source_loc_expr(kind, ty, ctx, loc, rparen);
        self.record_source_loc(e, loc, ctx, kind == SourceLocKind::SourceLocStruct);
        e
    }

    /// The type `std::source_location::__impl`, spelled with its scope as
    /// clang prints it (clang `LookupStdSourceLocationImpl`): it must be a
    /// complete standard-layout class without bases whose only fields are
    /// `const char *_M_file_name`, `const char *_M_function_name` and
    /// integral `_M_line` and `_M_column`.
    fn std_source_location_impl(&mut self, loc: Loc) -> Option<QualType> {
        let found = match self.srcloc.impl_decl {
            Some(d) => Some(d),
            None => self.lookup_source_location_impl(),
        };
        let Some(d) = found else {
            self.error(DiagId::sema_std_source_location_impl_not_found, loc);
            return None;
        };
        if self.srcloc.impl_decl.is_none() {
            if !self.source_location_impl_ok(d) {
                self.error(DiagId::sema_std_source_location_impl_malformed, loc);
                return None;
            }
            self.srcloc.impl_decl = Some(d);
        }
        let rec = self.ast.types.canonical(self.tag_type(d));
        let outer = self.ast.decls.get(d).ctx()?;
        let ns = self.ast.decls.get(outer).ctx()?;
        let outer_ty = self.tag_type(outer);
        let nns_ns = self.ast.types.nns(Nns::Namespace { prefix: None, ns: self.ast.first(ns) });
        let nns = self.ast.types.nns(Nns::Type {
            prefix: Some(nns_ns),
            ty: outer_ty,
            template_kw: false,
        });
        Some(self.ast.types.elaborated(TagKeyword::None, Some(nns), rec))
    }

    /// The complete class `std::source_location::__impl`, if declared.
    fn lookup_source_location_impl(&self) -> Option<DeclId> {
        let std = self.intern("std");
        let ns = self
            .lookup_in(self.ast.tu, std, LookupKind::Nested)
            .single()
            .filter(|&d| self.ast.decls.get(d).kind() == DeclKind::Namespace)?;
        let sl = self.intern("source_location");
        let outer = self.lookup_qualified(ns, sl, LookupKind::Tag).single()?;
        let outer = self.ast.definition(outer)?;
        let imp = self.intern("__impl");
        let d = self.lookup_qualified(outer, imp, LookupKind::Tag).single()?;
        let d = self.ast.definition(d)?;
        self.ast.decls.record(d)?;
        Some(d)
    }

    /// Whether `__impl` definition `d` has the shape clang requires.
    fn source_location_impl_ok(&mut self, d: DeclId) -> bool {
        let Some(r) = self.ast.decls.record(d) else { return false };
        if self.is_union(d)
            || !r.defn.contains(kiln_ast::decl::DefinitionBits::STANDARD_LAYOUT)
            || self.ast.decls.bases(d, &self.ast.lists).next().is_some()
        {
            return false;
        }
        let char_ptr = self.const_char_ptr(QualType::CHAR);
        let char_ptr = self.ast.types.canonical(char_ptr);
        let fields: Vec<DeclId> = self
            .ast
            .members(d)
            .filter(|&m| self.ast.decls.get(m).kind() == DeclKind::Field)
            .collect();
        let mut count = 0;
        for f in fields {
            let n = *self.ast.decls.get(f);
            let Some(t) = n.ty() else { return false };
            let t = self.ast.types.canonical(t);
            let name = match n.name().as_ident() {
                Some(s) => self.ast.symbols().as_str(s).to_owned(),
                None => return false,
            };
            let ok = match name.as_str() {
                "_M_file_name" | "_M_function_name" => t == char_ptr,
                "_M_line" | "_M_column" => self.ast.types.is_integral(t),
                _ => return false,
            };
            if !ok {
                return false;
            }
            count += 1;
        }
        count == 4
    }

    /// Records the values of every source-location builtin for a use `e`
    /// at `loc` in context `ctx` (clang `SourceLocExpr::EvaluateInContext`
    /// with `getExpansionRange(Loc).getEnd()`, kiln's expansion location);
    /// `location` builds the `__impl` object too.
    fn record_source_loc(&mut self, e: ExprId, loc: Loc, ctx: DeclId, location: bool) {
        let (file, line, column) = match self.diags.sm().presumed_line_col(loc) {
            Some(p) => (p.file_name.to_owned(), p.line, p.col),
            None => (String::new(), 0, 0),
        };
        let file_name = file.rsplit('/').next().unwrap_or(&file).to_owned();
        let (plain, pretty) = self.predefined_function_names(ctx);
        let Some(file_p) = self.srcloc_string(&file) else { return };
        let Some(file_name_p) = self.srcloc_string(&file_name) else { return };
        let Some(function_p) = self.srcloc_string(&plain) else { return };
        let location = if location {
            let Some(pretty_p) = self.srcloc_string(&pretty) else { return };
            self.source_location_object(file_p, pretty_p, line, column)
        } else {
            None
        };
        let v = SourceLocValues {
            file: file_p,
            file_name: file_name_p,
            function: function_p,
            line,
            column,
            location,
        };
        let _ = self.ast.exprs.set_source_loc_values(e, v);
    }

    /// The `const char *` value pointing to the string literal object
    /// holding `s` (one object per content).
    fn srcloc_string(&mut self, s: &str) -> Option<kiln_ast::ConstValueId> {
        let lit =
            self.ast.strings.intern(kiln_ast::strings::StringEncoding::Ordinary, s.as_bytes());
        let origin = match self.srcloc.strings.get(&lit) {
            Some(&e) => e,
            None => {
                let ty = crate::literal::string_literal_type(
                    self,
                    kiln_lex::Encoding::Ordinary,
                    s.len() as u64 + 1,
                );
                let e = self.builder().string_literal(
                    ty,
                    kiln_ast::expr::StringKind::Ordinary,
                    lit,
                    Loc::INVALID,
                );
                self.srcloc.strings.insert(lit, e);
                e
            }
        };
        let ty = self.const_char_ptr(QualType::CHAR);
        let ty = self.ast.types.canonical(ty);
        let base = PtrBase::StringLit { lit, origin };
        self.ast.consts.push_scalar(Scalar::Pointer { ty, base, offset: 0 }).ok()
    }

    /// The `const std::source_location::__impl *` value pointing to the
    /// frozen `__impl` object `{file, function, line, column}` (clang's
    /// `UnnamedGlobalConstantDecl`): equal values share one object.
    fn source_location_object(
        &mut self,
        file: kiln_ast::ConstValueId,
        function: kiln_ast::ConstValueId,
        line: u32,
        column: u32,
    ) -> Option<kiln_ast::ConstValueId> {
        let d = self.srcloc.impl_decl?;
        let rec = self.ast.types.canonical(self.tag_type(d));
        let fields: Vec<DeclId> = self
            .ast
            .members(d)
            .filter(|&m| self.ast.decls.get(m).kind() == DeclKind::Field)
            .collect();
        let layout = kiln_abi::ensure_layout(&mut self.ast, d, &self.abi);
        let size = layout.size as usize;
        let offsets: Vec<u64> = layout.fields.to_vec();
        let mut bytes = vec![0u8; size];
        let mut covered = vec![false; size];
        let mut relocs = Vec::new();
        let ptr_bytes = usize::from(self.ast.consts.pointer_bytes());
        for (f, off) in fields.into_iter().zip(offsets) {
            let at = (off / 8) as usize;
            let n = self.ast.decls.get(f);
            let name = n.name().as_ident().map(|s| self.ast.symbols().as_str(s).to_owned());
            let fty = n.ty().unwrap_or(QualType::UINT);
            match name.as_deref() {
                Some("_M_file_name" | "_M_function_name") => {
                    let value =
                        if name.as_deref() == Some("_M_file_name") { file } else { function };
                    relocs.push(kiln_ast::consts::Reloc { at: at as u32, value });
                    covered.get_mut(at..at + ptr_bytes)?.fill(true);
                }
                _ => {
                    let v = if name.as_deref() == Some("_M_line") { line } else { column };
                    let (w, _) = crate::target::size_align(&self.ast, fty, &self.abi)?;
                    let w = usize::try_from(w).ok()?;
                    let le = u64::from(v).to_le_bytes();
                    for i in 0..w {
                        *bytes.get_mut(at + i)? = le.get(i).copied().unwrap_or(0);
                    }
                    covered.get_mut(at..at + w)?.fill(true);
                }
            }
        }
        relocs.sort_by_key(|r| r.at);
        let mut padding = Vec::new();
        let mut i = 0;
        while i < size {
            if covered[i] {
                i += 1;
                continue;
            }
            let start = i;
            while i < size && !covered[i] {
                i += 1;
            }
            padding
                .push(kiln_ast::consts::ByteRange { start: start as u32, len: (i - start) as u32 });
        }
        let img = kiln_ast::consts::ByteImage {
            ty: rec,
            bytes: &bytes,
            relocs: &relocs,
            padding: &padding,
            unions: &[],
        };
        let obj = self.ast.consts.push_aggregate(img).ok()?;
        let cq = self.ast.types.qualify(rec, Quals::CONST);
        let pty = self.ast.types.pointer_to(cq);
        let pty = self.ast.types.canonical(pty);
        self.ast
            .consts
            .push_scalar(Scalar::Pointer { ty: pty, base: PtrBase::Const(obj), offset: 0 })
            .ok()
    }

    /// clang `PredefinedExpr::ComputeName` of context `ctx` for
    /// `__builtin_FUNCTION` (`Function`: the unqualified name) and
    /// `_M_function_name` (`PrettyFunction`); both empty outside a
    /// function. Gap: the ` [T = int]` suffix of template specializations
    /// is not printed (spec/sema-templates.md §9.5).
    fn predefined_function_names(&self, ctx: DeclId) -> (String, String) {
        let n = self.ast.decls.get(ctx);
        if !n.kind().is_function() {
            return (String::new(), String::new());
        }
        let names = kiln_ast::dump::AstNames::new(&self.ast);
        let mut plain = String::new();
        names.write_name(ctx, &mut plain);
        let mut pretty = String::new();
        if let Some(f) = self.ast.decls.function(ctx)
            && n.kind() == DeclKind::CXXMethod
        {
            if f.spec.is_virtual() {
                pretty.push_str("virtual ");
            }
            if f.spec.storage() == FnStorage::Static {
                pretty.push_str("static ");
            }
        }
        pretty.push_str(&kiln_abi::vtable::pretty_function(&self.ast, ctx));
        (plain, pretty)
    }

    /// Records the source-location values at a use of `param`'s default
    /// argument (the `CXXDefaultArgExpr` `e`, in the current context, for
    /// the call beginning at `call_begin`) when the argument evaluates a
    /// source-location builtin ([support.srcloc.cons]/2: the location of
    /// the call; clang `CXXDefaultArgExpr::getUsedLocation`).
    pub(crate) fn note_default_arg_use(&mut self, e: ExprId, param: DeclId, call_begin: Loc) {
        let kinds = self.default_arg_srcloc_kinds(param);
        if kinds != 0 {
            let ctx = self.ctx;
            self.record_source_loc(e, call_begin, ctx, kinds & SRC_LOC_STRUCT_BIT != 0);
        }
    }

    /// The source-location builtins `param`'s default argument evaluates
    /// (a mask of `1 << kind`), through nested default arguments.
    fn default_arg_srcloc_kinds(&mut self, param: DeclId) -> u8 {
        if let Some(&k) = self.srcloc.param_kinds.get(&param) {
            return k;
        }
        // Guards recursion through `void f(int = f())`.
        self.srcloc.param_kinds.insert(param, 0);
        let Some(init) = self.ast.decls.param(param).and_then(|p| p.default_arg) else {
            return 0;
        };
        let mut kinds = 0u8;
        let mut stack = vec![init];
        while let Some(x) = stack.pop() {
            match self.ast.exprs.get(x).kind() {
                ExprKind::SourceLocExpr => {
                    kinds |= 1 << self.ast.exprs.source_loc_kind(x) as u8;
                }
                ExprKind::CXXDefaultArgExpr => {
                    let p = self.ast.exprs.default_arg_param(x);
                    kinds |= self.default_arg_srcloc_kinds(p);
                }
                _ => stack.extend(self.ast.exprs.children(x, &self.ast.lists)),
            }
        }
        self.srcloc.param_kinds.insert(param, kinds);
        kinds
    }
}
