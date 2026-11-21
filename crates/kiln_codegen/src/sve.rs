//! Calls of the SVE and SME intrinsics `arm_sve.h`, `arm_sme.h` and
//! `arm_neon_sve_bridge.h` declare as aliases of clang builtins
//! (`__attribute__((__clang_arm_builtin_alias(B)))`, WP-810, WP-1739;
//! spec/landed/codegen/WP-810.md "Builtin aliases").
//!
//! A call of such a function is a call of builtin `B`, never of an
//! external function. It is lowered as clang `EmitAArch64SVEBuiltinExpr`
//! / `EmitAArch64SMEBuiltinExpr` lower it, by interpreting the builtin's
//! lowering in the generated table ([`kiln_ast::sve`]): clang 22's at
//! `-fclang-compat=20` and later, clang 19's below, each intrinsic declared
//! with the attributes of the LLVM version the IR is for. Sema accepts
//! only the table's builtins as an alias, and checks the call's streaming
//! mode, state and immediates (WP-1792). The calling function needs the
//! builtin's required features (WP-1793; clang `checkTargetFeatures`,
//! `err_builtin_needs_feature`: [`sve::checks`]' expression).

use kiln_ast::sve::{self, Lowering};
use kiln_ast::{DeclId, ExprId};
use kiln_diag::{Diag, DiagId};
use kiln_llvm::{
    Attr, AttrKind, BinOp, CastOp, ConstData, Flags, IntPredicate, MemoryEffects, ModRef,
    TypeId as LTy, TypeTable, intrinsics,
};
use kiln_source::Span;
use kiln_source::symbol::Symbol;

use crate::fir::{Op, V};
use crate::module::{Cg, R, err};

/// The abbreviated LLVM type `s` of the table ([`sve::type_text`]).
fn parse_type(tt: &mut TypeTable, s: &str) -> Option<LTy> {
    Some(match s {
        "void" => LTy::VOID,
        "h" => LTy::HALF,
        "bf" => LTy::BFLOAT,
        "f" => LTy::FLOAT,
        "d" => LTy::DOUBLE,
        "p" => tt.ptr(0),
        "c" => tt.target_ext("aarch64.svcount"),
        _ => {
            if let Some(f) = s.strip_prefix('{').and_then(|x| x.strip_suffix('}')) {
                let fields: Option<Vec<LTy>> = f.split(',').map(|x| parse_type(tt, x)).collect();
                return Some(tt.struct_literal(&fields?, false));
            }
            if let Some(n) = s.strip_prefix('i') {
                return Some(tt.int(n.parse().ok()?));
            }
            let scalable = s.starts_with('s');
            let rest = s.strip_prefix('s').or_else(|| s.strip_prefix('v'))?;
            let digits = rest.find(|c: char| !c.is_ascii_digit())?;
            let n: u32 = rest[..digits].parse().ok()?;
            let e = parse_type(tt, &rest[digits..])?;
            if scalable { tt.scalable_vector(e, n) } else { tt.vector(e, n) }
        }
    })
}

/// The function attributes `s` of the table (`nocallback nofree ...
/// memory(argmem: read)`).
fn parse_attrs(s: &str) -> Option<Vec<Attr>> {
    let mut out = Vec::new();
    let (flags, mem) = match s.find("memory(") {
        Some(i) => (&s[..i], Some(s[i + 7..].strip_suffix(')')?)),
        None => (s, None),
    };
    for w in flags.split_whitespace() {
        out.push(Attr::Flag(match w {
            "nocallback" => AttrKind::NoCallback,
            "nofree" => AttrKind::NoFree,
            "nosync" => AttrKind::NoSync,
            "nounwind" => AttrKind::NoUnwind,
            "speculatable" => AttrKind::Speculatable,
            "willreturn" => AttrKind::WillReturn,
            _ => return None,
        }));
    }
    if let Some(mem) = mem {
        let modref = |x: &str| match x {
            "none" => Some(ModRef::None),
            "read" => Some(ModRef::Read),
            "write" => Some(ModRef::Write),
            "readwrite" => Some(ModRef::ReadWrite),
            _ => None,
        };
        let mut me = MemoryEffects::default();
        for part in mem.split(", ") {
            match part.split_once(": ") {
                Some(("argmem", m)) => me.argmem = modref(m)?,
                Some(("inaccessiblemem", m)) => me.inaccessiblemem = modref(m)?,
                Some(_) => return None,
                None => me = MemoryEffects::all(modref(part)?),
            }
        }
        out.push(Attr::Memory(me));
    }
    Some(out)
}

/// The parameter attributes `c` of the table (`.`, `i` `immarg`, `c`
/// `nocapture`).
fn param_attrs(c: char) -> &'static [Attr] {
    match c {
        'i' => &[Attr::Flag(AttrKind::ImmArg)],
        'c' => &[Attr::Flag(AttrKind::NoCapture)],
        _ => &[],
    }
}

/// The clang version whose lowering kiln follows at `-fclang-compat=compat`:
/// clang 22's from 20 on (the tuples are structs since clang 20).
fn version(compat: u32) -> u32 {
    if compat >= 20 { 22 } else { 19 }
}

/// The immediate operands of builtin `name` (`#` in its signature), which
/// the constant-folding prepass folds.
pub(crate) fn imm_args(name: &str, compat: u32) -> Vec<usize> {
    let Some(l) = sve::lookup(name, version(compat)) else { return Vec::new() };
    let sig = l.family.split(';').next().unwrap_or("");
    sig.split(' ').skip(2).enumerate().filter(|(_, t)| *t == "#").map(|x| x.0).collect()
}

/// The result of an operation or of a whole lowering.
enum Val {
    /// A value.
    V(V),
    /// None (a `void` intrinsic call, a `void` builtin).
    Void,
}

impl Val {
    fn get(self) -> Option<V> {
        match self {
            Val::V(v) => Some(v),
            Val::Void => None,
        }
    }
}

/// A builtin's lowering being interpreted.
struct Run {
    /// The row's types.
    types: Vec<LTy>,
    /// The row's intrinsics ([`sve::intrinsic`] indexes).
    intrinsics: Vec<usize>,
    /// The immediate arguments' values.
    imms: Vec<Option<i128>>,
    /// The values of the operations so far.
    vals: Vec<Option<V>>,
}

impl Cg<'_> {
    /// The call `e` of `d`, an alias of builtin `b` (WP-810, WP-1739):
    /// lowered as clang. Never an external call.
    pub(crate) fn emit_sve_alias(&mut self, d: DeclId, b: Symbol, e: ExprId) -> R<Option<V>> {
        let ast = self.ast;
        let name = ast.symbols().as_str(b).to_owned();
        let on = self.feature_map(self.f.decl);
        // WP-1793: the builtin's own required-feature expression.
        let req =
            sve::checks(&name, version(self.opts.clang_compat)).map_or("sve|sme", |c| c.features);
        if !sve::features_hold(req, &|n| on.get(n) == Some(&true)) {
            // clang `checkTargetFeatures`: the error names the function
            // called, then the call is emitted as usual (an operand's own
            // builtin call is checked too).
            let called =
                ast.decls.get(d).name().as_ident().map(|s| ast.symbols().as_str(s).to_owned());
            let diag = Diag::new(DiagId::cg_builtin_needs_feature, Span::empty(ast.begin_loc(e)))
                .arg(called.unwrap_or_default())
                .arg(req.to_owned());
            self.diags.push(diag);
        }
        let Some(l) = sve::lookup(&name, version(self.opts.clang_compat)) else {
            unreachable!("sema accepts only the SVE and SME builtins of the table: {name}")
        };
        let args: Vec<ExprId> = ast.lists.iter(ast.exprs.call_args(e)).collect();
        match self.sve_run(l, &name, &args, e)? {
            Some(v) => Ok(v.get()),
            None => err(self.loc_of(e), format!("internal error: malformed lowering of '{name}'")),
        }
    }

    /// Interprets lowering `l` of builtin `name` for call `e` (`None`: a
    /// malformed table entry).
    fn sve_run(&mut self, l: Lowering, name: &str, args: &[ExprId], e: ExprId) -> R<Option<Val>> {
        let mut types = Vec::new();
        for i in l.types.split(',').filter(|s| !s.is_empty()) {
            let Some(t) = i.parse().ok().and_then(sve::type_text) else { return Ok(None) };
            let Some(t) = parse_type(&mut self.m.types, t) else { return Ok(None) };
            types.push(t);
        }
        let intrinsics: Option<Vec<usize>> =
            l.intrinsics.split(',').filter(|s| !s.is_empty()).map(|s| s.parse().ok()).collect();
        let Some(intrinsics) = intrinsics else { return Ok(None) };
        let mut run = Run { types, intrinsics, imms: vec![None; args.len()], vals: Vec::new() };
        let ops: Vec<&str> = l.family.split(';').collect();
        let Some(sig) = ops.first().and_then(|s| s.strip_prefix("S ")) else { return Ok(None) };
        // `arm_sve.h` declares each alias with its builtin's signature; any
        // other declaration is an error (clang emits an invalid call).
        let Some(ok) = self.sve_signature(&run, sig, args, e) else { return Ok(None) };
        if !ok {
            return err(
                self.loc_of(e),
                format!("the function called does not have the signature of SVE builtin '{name}'"),
            );
        }
        // An immediate is a constant expression (clang
        // `EmitScalarOrConstFoldImmArg`); nothing is emitted for it.
        for (k, t) in sig.split(' ').skip(1).enumerate() {
            if t == "#" {
                let Some(v) = self.fold_int(args[k]) else {
                    // clang's wording, at the call: Sema reports it first
                    // (WP-1792), so this is not reached.
                    let ast = self.ast;
                    let called = crate::builtins::direct_fn(ast, e)
                        .and_then(|d| ast.decls.get(d).name().as_ident())
                        .map_or_else(|| name.to_owned(), |s| ast.symbols().as_str(s).to_owned());
                    return err(
                        Some(ast.begin_loc(e)),
                        format!("argument to '{called}' must be a constant integer"),
                    );
                };
                run.imms[k] = Some(v);
            }
        }
        // The `D` operations are declared in table order, each when a call
        // first needs it or a later one (found by WP-1794): clang emits every
        // operand (`GetAArch64SVEProcessedOperands`) before it creates
        // the builtin's intrinsics, so a call nested in an operand
        // (`svadd_z(svptrue_b32(), a, b)`) declares its intrinsics first.
        let mut specs = Vec::new();
        let mut decls = Vec::new();
        let mut steps = Vec::new();
        for op in &ops[1..] {
            match op.strip_prefix("D ") {
                Some(d) => specs.push(d),
                None => steps.push(*op),
            }
        }
        let mut i = 0;
        while i < steps.len() {
            let w: Vec<&str> = steps[i].split(' ').collect();
            let need = match (w[0], w.get(1)) {
                ("c", Some(k)) => k.parse::<usize>().ok().map(|k| k + 1),
                ("r", _) => Some(specs.len()),
                _ => None,
            };
            while need.is_some_and(|n| decls.len() < n.min(specs.len())) {
                let Some(f) = self.sve_declare(&run, specs[decls.len()]) else { return Ok(None) };
                decls.push(f);
            }
            let v = match (w[0], &w[1..]) {
                ("a", [k]) => {
                    let Some(&a) = k.parse::<usize>().ok().and_then(|k| args.get(k)) else {
                        return Ok(None);
                    };
                    Some(self.emit_scalar(a)?)
                }
                ("r", []) => return Ok(Some(Val::Void)),
                ("r", [v]) => {
                    let Some(v) = self.sve_value(&run, v) else { return Ok(None) };
                    return Ok(Some(Val::V(v)));
                }
                ("ie", [vec, elt, idx]) => {
                    // clang `CreateVectorSplat` (an insertion into
                    // element 0 of poison then a zero-mask shuffle) folds
                    // a constant: [`Cg::splat`].
                    let splat = steps.get(i + 1).is_some_and(|n| {
                        n.starts_with(&format!("sv %{i} "))
                            && n.ends_with(":Z")
                            && vec.ends_with(":P")
                    });
                    let (Some(vv), Some(ev), Some(iv)) = (
                        self.sve_value(&run, vec),
                        self.sve_value(&run, elt),
                        self.sve_value(&run, idx),
                    ) else {
                        return Ok(None);
                    };
                    if splat && self.cint(iv).is_some_and(|x| x.1 == 0) {
                        let vt = self.f.fir.ty(&self.m, vv);
                        run.vals.push(None);
                        run.vals.push(Some(self.splat(ev, vt)));
                        i += 2;
                        continue;
                    }
                    Some(self.insert_elt(vv, ev, iv))
                }
                _ => {
                    let Some(v) = self.sve_step(&run, &decls, w[0], &w[1..]) else {
                        return Ok(None);
                    };
                    v.get()
                }
            };
            run.vals.push(v);
            i += 1;
        }
        // A lowering ends with its `r` operation.
        Ok(None)
    }

    /// Whether the call `e` with arguments `args` matches signature `sig`
    /// (`ret params...`, `#` an immediate of integer type).
    fn sve_signature(&mut self, run: &Run, sig: &str, args: &[ExprId], e: ExprId) -> Option<bool> {
        let ast = self.ast;
        let mut it = sig.split(' ');
        let ret = self.sve_type(run, it.next()?)?;
        let params: Vec<&str> = it.collect();
        let mut ok = args.len() == params.len() && self.lty(ast.exprs.get(e).ty()) == ret;
        for (&a, p) in args.iter().zip(&params) {
            let t = self.lty(ast.exprs.get(a).ty());
            ok &= if *p == "#" { self.m.types.is_int(t) } else { self.sve_type(run, p)? == t };
        }
        Some(ok)
    }

    /// The type token `t` (`$n`, `i32`, `i64`, `p`, `void`).
    fn sve_type(&mut self, run: &Run, t: &str) -> Option<LTy> {
        match t.strip_prefix('$') {
            Some(n) => run.types.get(n.parse::<usize>().ok()?).copied(),
            None => parse_type(&mut self.m.types, t),
        }
    }

    /// The value token `x`: `%n`, `T:k` or `T#k[*s][+o]`.
    fn sve_value(&mut self, run: &Run, x: &str) -> Option<V> {
        if let Some(n) = x.strip_prefix('%') {
            return *run.vals.get(n.parse::<usize>().ok()?)?;
        }
        if let Some((t, imm)) = x.split_once('#') {
            let ty = self.sve_type(run, t)?;
            let (k, rest) = imm.split_at(imm.find(['*', '+']).unwrap_or(imm.len()));
            let mut v = (*run.imms.get(k.parse::<usize>().ok()?)?)?;
            let (s, o) = match rest.split_once('+') {
                Some((s, o)) => (s, o.parse::<i128>().ok()?),
                None => (rest, 0),
            };
            if let Some(s) = s.strip_prefix('*') {
                v = v.checked_mul(s.parse::<i128>().ok()?)?;
            }
            return Some(V::C(self.m.const_int(ty, v.checked_add(o)?)));
        }
        let (t, k) = x.split_once(':')?;
        let ty = self.sve_type(run, t)?;
        Some(V::C(match k {
            "P" => self.m.const_poison(ty),
            "U" => self.m.const_undef(ty),
            "Z" => self.m.const_zero(ty),
            _ => self.m.const_int(ty, k.parse().ok()?),
        }))
    }

    /// Declares the intrinsic of `D` operation `d` (`@k[.T...] ret
    /// params...`) with the attributes of the LLVM version the IR is for.
    fn sve_declare(&mut self, run: &Run, d: &str) -> Option<kiln_llvm::FuncId> {
        let mut w = d.split(' ');
        let mut name = w.next()?.strip_prefix('@')?.split('.');
        let x = sve::intrinsic(*run.intrinsics.get(name.next()?.parse::<usize>().ok()?)?)?;
        let mut overloads = Vec::new();
        for t in name {
            overloads.push(self.sve_type(run, t)?);
        }
        let full = intrinsics::overloaded_name(&self.m, x.name, &overloads);
        let ret = self.sve_type(run, w.next()?)?;
        let v = usize::from(self.m.ir_version < 22 && x.attrs[1].is_some());
        let fattrs = parse_attrs(x.attrs[v].or(x.attrs[1 - v])?)?;
        let mut pa = x.params[v].chars();
        let mut params = Vec::new();
        for t in w {
            params.push((self.sve_type(run, t)?, param_attrs(pa.next().unwrap_or('.'))));
        }
        Some(intrinsics::declare_with_attrs(&mut self.m, &full, ret, &params, &fattrs))
    }

    /// One instruction `op` with operands `w` (`None`: malformed).
    fn sve_step(
        &mut self,
        run: &Run,
        decls: &[kiln_llvm::FuncId],
        op: &str,
        w: &[&str],
    ) -> Option<Val> {
        let val = |cg: &mut Self, i: usize| cg.sve_value(run, w.get(i)?);
        // An aggregate index: a number, or immediate `#k` (`svget2`).
        let idx = |i: usize| {
            let x = w.get(i)?;
            match x.strip_prefix('#') {
                Some(k) => u32::try_from((*run.imms.get(k.parse::<usize>().ok()?)?)?).ok(),
                None => x.parse::<u32>().ok(),
            }
        };
        Some(Val::V(match op {
            "c" => {
                let f = *decls.get(w.first()?.parse::<usize>().ok()?)?;
                let args: Option<Vec<V>> = (1..w.len()).map(|i| val(self, i)).collect();
                let v = self.intrinsic_call(f, args?);
                let fty = self.m.func(f).ty();
                if self.m.types.function_info(fty).is_some_and(|i| i.0 == LTy::VOID) {
                    return Some(Val::Void);
                }
                v
            }
            "ee" => {
                let vec = val(self, 0)?;
                let i = val(self, 1)?;
                self.extract_elt(vec, i)
            }
            "ev" => {
                let agg = val(self, 0)?;
                self.sve_extract_value(agg, idx(1)?)?
            }
            "iv" => {
                let agg = val(self, 0)?;
                let v = val(self, 1)?;
                self.sve_insert_value(agg, v, idx(2)?)?
            }
            "sv" => {
                let a = val(self, 0)?;
                let b = val(self, 1)?;
                let mask = w.get(2)?.strip_suffix(":Z")?;
                let mt = self.sve_type(run, mask)?;
                let n = self.m.types.vector_info(mt)?.1;
                self.shuffle(a, Some(b), vec![0; n as usize])
            }
            "sel" => {
                let c = val(self, 0)?;
                let t = val(self, 1)?;
                let f = val(self, 2)?;
                self.select(c, t, f)
            }
            "gep" => {
                let t = self.sve_type(run, w.first()?)?;
                let base = val(self, 1)?;
                let i = val(self, 2)?;
                self.gep(t, base, vec![i], false)
            }
            "zext" | "sext" | "trunc" | "bitcast" => {
                let c = match op {
                    "zext" => CastOp::ZExt,
                    "sext" => CastOp::SExt,
                    "trunc" => CastOp::Trunc,
                    _ => CastOp::BitCast,
                };
                let v = val(self, 0)?;
                let t = self.sve_type(run, w.get(1)?)?;
                self.cast(c, v, t)
            }
            "icmp.ne" => {
                let a = val(self, 0)?;
                let b = val(self, 1)?;
                self.icmp(IntPredicate::Ne, a, b)
            }
            _ => {
                let mut it = op.split('.');
                let bin = match it.next()? {
                    "shl" => BinOp::Shl,
                    "mul" => BinOp::Mul,
                    "add" => BinOp::Add,
                    _ => return None,
                };
                let mut flags = Flags::NONE;
                for f in it {
                    flags = flags.with(match f {
                        "nuw" => Flags::NUW,
                        "nsw" => Flags::NSW,
                        _ => return None,
                    });
                }
                let a = val(self, 0)?;
                let b = val(self, 1)?;
                self.bin(bin, flags, a, b)
            }
        }))
    }

    /// The field types of struct value `agg` and its constant kind, when
    /// it is a constant `poison`, `undef` or `zeroinitializer`.
    fn sve_const_agg(&mut self, agg: V) -> Option<(Vec<LTy>, u8)> {
        let V::C(c) = agg else { return None };
        let kind = match self.m.const_of(c)? {
            ConstData::Poison(_) => b'P',
            ConstData::Undef(_) => b'U',
            ConstData::Zero(_) => b'Z',
            _ => return None,
        };
        let t = self.m.const_type(c);
        Some((self.m.types.struct_fields(t)?.to_vec(), kind))
    }

    fn sve_const_of_kind(&mut self, t: LTy, kind: u8) -> kiln_llvm::Value {
        match kind {
            b'P' => self.m.const_poison(t),
            b'U' => self.m.const_undef(t),
            _ => self.m.const_zero(t),
        }
    }

    /// `extractvalue agg, i` (`IRBuilder` folds a constant aggregate).
    fn sve_extract_value(&mut self, agg: V, i: u32) -> Option<V> {
        let t = self.f.fir.ty(&self.m, agg);
        let ft = *self.m.types.struct_fields(t)?.get(i as usize)?;
        if let Some((_, kind)) = self.sve_const_agg(agg) {
            return Some(V::C(self.sve_const_of_kind(ft, kind)));
        }
        Some(self.f.fir.push(Op::ExtractValue { agg, idx: i }, ft))
    }

    /// `insertvalue agg, v, i` (`IRBuilder` folds constants).
    fn sve_insert_value(&mut self, agg: V, v: V, i: u32) -> Option<V> {
        let t = self.f.fir.ty(&self.m, agg);
        if let (Some((fields, kind)), V::C(c)) = (self.sve_const_agg(agg), v) {
            let elems: Vec<_> = fields
                .iter()
                .enumerate()
                .map(|(j, &ft)| if j == i as usize { c } else { self.sve_const_of_kind(ft, kind) })
                .collect();
            if let Ok(k) = self.m.const_struct(t, &elems) {
                return Some(V::C(k));
            }
        }
        Some(self.f.fir.push(Op::InsertValue { agg, val: v, idx: i }, t))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every row's types, intrinsics and attributes are well formed.
    #[test]
    fn table() {
        let mut tt = TypeTable::new();
        for i in 0.. {
            let Some(t) = sve::type_text(i) else { break };
            assert!(parse_type(&mut tt, t).is_some(), "{t}");
        }
        for i in 0.. {
            let Some(x) = sve::intrinsic(i) else { break };
            for a in x.attrs.into_iter().flatten() {
                assert!(parse_attrs(a).is_some(), "{a}");
            }
        }
        assert_eq!(
            parse_attrs("nounwind memory(argmem: readwrite, inaccessiblemem: read)"),
            Some(vec![
                Attr::Flag(AttrKind::NoUnwind),
                Attr::Memory(MemoryEffects {
                    argmem: ModRef::ReadWrite,
                    inaccessiblemem: ModRef::Read,
                    other: ModRef::None
                })
            ])
        );
    }
}
