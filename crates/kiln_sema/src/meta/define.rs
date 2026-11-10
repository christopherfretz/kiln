//! `[meta.reflection.define.aggregate]` (spec/reflection.md §6):
//! `data_member_spec` validates a `data_member_options` (read by the
//! Oven, `kiln_oven/src/meta/define.rs`) into an interned description,
//! and `define_aggregate` completes an incomplete class with the members
//! the descriptions give, in one pass, while the consteval block
//! evaluates. `__kiln_define_member` (`ext.rs`) injects through the same
//! path ([`Sema::inject_field`]).

use kiln_ast::annot::Annotation;
use kiln_ast::decl::{Access, DeclFlags, DeclKind, FieldData};
use kiln_ast::expr::Radix;
use kiln_ast::names::DeclName;
use kiln_ast::reflect::Reflectee;
use kiln_ast::types::{ArgView, BuiltinType};
use kiln_ast::{DeclHead, DeclId, QualType, ReflectionId};
use kiln_diag::DiagId;
use kiln_oven::{DefineOp, Dms, MetaCtx, MetaError, MetaOut, Note, OptionsPlan, Oven, info_arg};
use kiln_source::Loc;
use kiln_source::fx::FxHashSet;
use kiln_source::symbol::Symbol;

use super::fail;
use crate::Sema;
use crate::lookup::LookupKind;

/// The keywords and alternative tokens of C++26 ([lex.key], tables 5
/// and 6), sorted: none is an identifier ([lex.name]/3).
const KEYWORDS: [&str; 93] = [
    "alignas",
    "alignof",
    "and",
    "and_eq",
    "asm",
    "auto",
    "bitand",
    "bitor",
    "bool",
    "break",
    "case",
    "catch",
    "char",
    "char16_t",
    "char32_t",
    "char8_t",
    "class",
    "co_await",
    "co_return",
    "co_yield",
    "compl",
    "concept",
    "const",
    "const_cast",
    "consteval",
    "constexpr",
    "constinit",
    "continue",
    "contract_assert",
    "decltype",
    "default",
    "delete",
    "do",
    "double",
    "dynamic_cast",
    "else",
    "enum",
    "explicit",
    "export",
    "extern",
    "false",
    "float",
    "for",
    "friend",
    "goto",
    "if",
    "inline",
    "int",
    "long",
    "mutable",
    "namespace",
    "new",
    "noexcept",
    "not",
    "not_eq",
    "nullptr",
    "operator",
    "or",
    "or_eq",
    "private",
    "protected",
    "public",
    "register",
    "reinterpret_cast",
    "requires",
    "return",
    "short",
    "signed",
    "sizeof",
    "static",
    "static_assert",
    "static_cast",
    "struct",
    "switch",
    "template",
    "this",
    "thread_local",
    "throw",
    "true",
    "try",
    "typedef",
    "typeid",
    "typename",
    "union",
    "unsigned",
    "using",
    "virtual",
    "void",
    "volatile",
    "wchar_t",
    "while",
    "xor",
    "xor_eq",
];

/// The largest alignment or bit-field width a description holds (both
/// are `int` in `data_member_options`; anything larger is rejected).
const MAX_VALUE: i64 = 1 << 28;

/// Whether `s` is a keyword or an alternative token.
pub(crate) fn is_keyword(s: &str) -> bool {
    KEYWORDS.binary_search(&s).is_ok()
}

impl Sema<'_, '_> {
    pub(super) fn meta_define(
        &mut self,
        d: DefineOp,
        args: &[u128],
        out: &mut MetaOut,
        ctx: &MetaCtx,
        oven: &mut Oven,
    ) -> Result<(), MetaError> {
        *out = match d {
            DefineOp::DataMemberSpec => MetaOut::Info(self.data_member_spec(args, ctx, oven)?),
            DefineOp::IsDataMemberSpec => {
                let r = info_arg(args, 0)?;
                let is = matches!(self.ast.reflections.get(r), Reflectee::DataMemberSpec(_));
                MetaOut::Scalar(u128::from(is))
            }
            DefineOp::DefineAggregate => MetaOut::Info(self.define_aggregate(args, ctx, oven)?),
        };
        Ok(())
    }

    /// How `std::meta::data_member_options` is read (spec/reflection.md
    /// §6.1), found once and cached in the Oven: its members `name`,
    /// `alignment`, `bit_width` (each an `optional`, whose payload is its
    /// first template argument) and `no_unique_address`.
    fn dms_plan(&mut self, oven: &mut Oven, loc: Loc) -> Result<OptionsPlan, MetaError> {
        if let Some(p) = oven.dms_plan() {
            return Ok(p);
        }
        let missing = || {
            MetaError::note(loc, "kiln: 'std::meta::data_member_options' has no layout kiln reads")
        };
        let ns = |s: &Self, ctx: DeclId, n: &str| {
            let n = s.ast.symbols().intern(n);
            s.lookup_qualified(ctx, n, LookupKind::Nested)
                .single()
                .filter(|&d| s.kind_of(d) == DeclKind::Namespace)
        };
        let std = ns(self, self.ast.tu, "std").ok_or_else(missing)?;
        let meta = ns(self, std, "meta").ok_or_else(missing)?;
        let n = self.ast.symbols().intern("data_member_options");
        let cls = self.lookup_qualified(meta, n, LookupKind::Tag).single().ok_or_else(missing)?;
        let def = self.ast.definition(cls).ok_or_else(missing)?;
        let class = self.ast.types.canonical(self.tag_type(def)).without_quals();
        let field = |s: &Self, n: &str| -> Option<(DeclId, QualType)> {
            s.ast.members(def).find_map(|m| {
                let node = s.ast.decls.get(m);
                (node.kind() == DeclKind::Field && s.decl_name(m) == n)
                    .then(|| node.ty())
                    .flatten()
                    .map(|t| (m, t))
            })
        };
        let optional = |s: &Self, n: &str| -> Option<(DeclId, QualType)> {
            let (m, t) = field(s, n)?;
            let r = s.ast.types.as_record(t)?;
            let (_, info) = s.ast.spec_info(r)?;
            let arg = s.ast.types.args(info.args).first()?;
            let ArgView::Type(p) = arg.view() else { return None };
            Some((m, s.ast.types.canonical(p).without_quals()))
        };
        let plan = OptionsPlan {
            class,
            name: optional(self, "name").ok_or_else(missing)?,
            alignment: optional(self, "alignment").ok_or_else(missing)?,
            bit_width: optional(self, "bit_width").ok_or_else(missing)?,
            nua: field(self, "no_unique_address").ok_or_else(missing)?.0,
            annotations: field(self, "annotations").map(|x| x.0),
        };
        oven.set_dms_plan(plan);
        Ok(plan)
    }

    /// `data_member_spec(type, options)` ([meta.reflection.define.aggregate]/
    /// 3-5): GCC 16.1's checks, in its order (spec/reflection.md §6.1);
    /// each failure is a `std::meta::exception`.
    fn data_member_spec(
        &mut self,
        args: &[u128],
        ctx: &MetaCtx,
        oven: &mut Oven,
    ) -> Result<ReflectionId, MetaError> {
        let loc = ctx.loc;
        let r = info_arg(args, 0)?;
        let addr = *args.get(1).ok_or_else(MetaError::invalid)?;
        let Reflectee::Type(t) = self.ast.reflections.get(r) else {
            return fail(loc, format!("'{}' does not reflect a type", self.reflection_str(r)));
        };
        let plan = self.dms_plan(oven, loc)?;
        let raw = oven.meta_dms_options(addr, &plan)?;
        let ty = self.ast.types.canonical(t);
        let types = &self.ast.types;
        if types.is_void(ty) || types.is_function(ty) {
            return fail(loc, format!("'{}' is not an object or reference type", self.ty_str(ty)));
        }
        let name = if raw.name_u8.is_some() {
            let bytes = oven.dms_name_bytes();
            let Some(s) = std::str::from_utf8(bytes).ok().filter(|s| kiln_lex::is_identifier(s))
            else {
                let shown = kiln_diag::escape_diag_text(bytes);
                return fail(loc, format!("'{shown}' is not a valid identifier"));
            };
            if is_keyword(s) {
                return fail(loc, format!("'{s}' is a keyword, not an identifier"));
            }
            Some(self.ast.symbols().intern(s))
        } else {
            None
        };
        if name.is_none() && raw.bit_width.is_none() {
            return fail(loc, "neither 'name' nor 'bit_width' specified");
        }
        if raw.alignment.is_some() && raw.bit_width.is_some() {
            return fail(loc, "both 'alignment' and 'bit_width' specified");
        }
        let alignment = match raw.alignment {
            None => None,
            Some(a) if a <= 0 || a > MAX_VALUE || a.count_ones() != 1 => {
                return fail(loc, format!("'alignment' {a} is not a positive power of two"));
            }
            Some(a) => {
                let reference = self.ast.types.is_reference(ty);
                if !reference && self.is_complete(ty) {
                    let natural =
                        crate::target::size_align(&self.ast, ty, &self.abi).map_or(0, |x| x.1);
                    if (a as u64) < natural {
                        return fail(
                            loc,
                            format!(
                                "'alignment' {a} is smaller than the alignment {natural} of '{}'",
                                self.ty_str(ty)
                            ),
                        );
                    }
                }
                Some(a as u32)
            }
        };
        let bit_width = match raw.bit_width {
            None => None,
            Some(_) if !self.is_integral_or_enum(ty) => {
                return fail(
                    loc,
                    format!(
                        "'bit_width' specified with non-integral and non-enumeration type '{}'",
                        self.ty_str(ty)
                    ),
                );
            }
            Some(_) if raw.nua => {
                return fail(loc, "'bit_width' specified with 'no_unique_address'");
            }
            Some(w) if w < 0 => return fail(loc, format!("'bit_width' {w} is negative")),
            Some(w) if w > MAX_VALUE => {
                return fail(loc, format!("'bit_width' {w} is too large"));
            }
            Some(0) if name.is_some() => {
                return fail(loc, "'bit_width' 0 specified with a 'name'");
            }
            Some(w) => {
                if name.is_none() && ty.quals_bits() != 0 {
                    return fail(
                        loc,
                        format!("unnamed bit-field of cv-qualified type '{}'", self.ty_str(ty)),
                    );
                }
                Some(w as u32)
            }
        };
        let annots = self.dms_annotations(oven, raw.annotations, ctx)?;
        let id = oven.dms_intern(Dms { ty, name, alignment, bit_width, nua: raw.nua, annots });
        Ok(self.intern_reflection(Reflectee::DataMemberSpec(id), loc)?)
    }

    /// The `annotations` of the options just read (P3394R4; GCC 16.1):
    /// each must have a type ("reflection does not have a type"), and
    /// stands for its constant (`constant_of`: a variable or object is
    /// its value, an annotation its underlying constant). The constants'
    /// list, interned (0 when there are none).
    fn dms_annotations(
        &mut self,
        oven: &mut Oven,
        n: u32,
        ctx: &MetaCtx,
    ) -> Result<u32, MetaError> {
        let loc = ctx.loc;
        if n == 0 {
            return Ok(0);
        }
        let args = oven.dms_annotation_args().to_vec();
        let mut out = Vec::with_capacity(args.len());
        for r in args {
            let x = self.ast.reflections.get(r);
            let c = match x {
                Reflectee::Value(_) => r,
                Reflectee::Object(_) if self.class_constant(r).is_some() => r,
                Reflectee::Annotation(a) => self.annotation_constant(a, loc)?,
                _ if self.type_of_reflectee(x).is_none() => {
                    return fail(loc, "reflection does not have a type");
                }
                _ => self.reflect_constant_of(r, ctx, oven)?,
            };
            out.push(c);
        }
        Ok(oven.dms_annots_intern(&out))
    }

    /// The class value of constant reflection `r` (an object reflection
    /// of a template parameter object, [temp.param]/8).
    fn class_constant(&self, r: ReflectionId) -> Option<kiln_ast::ConstValueId> {
        let Reflectee::Object(p) = self.ast.reflections.get(r) else { return None };
        match self.ast.consts.scalar(p) {
            Some(kiln_ast::consts::Scalar::Pointer {
                base: kiln_ast::consts::PtrBase::Decl(d),
                offset: 0,
                ..
            }) => self.ast.templates.param_object_value(d),
            _ => None,
        }
    }

    /// `define_aggregate(class_type, mdescrs)`
    /// ([meta.reflection.define.aggregate]/7-10; spec/reflection.md §6.2).
    /// Its preconditions are "Constant When" ones: a failure is not a
    /// constant expression (never a `std::meta::exception`). Checked in
    /// order: a plainly constant-evaluated call; an incomplete class not
    /// being defined; no function or class scope between the class's and
    /// the consteval block's; descriptions with complete types and
    /// distinct names, in one pass. The class is then defined (its
    /// members [`DeclFlags::INJECTED`]) and completed, once.
    fn define_aggregate(
        &mut self,
        args: &[u128],
        ctx: &MetaCtx,
        oven: &mut Oven,
    ) -> Result<ReflectionId, MetaError> {
        let loc = ctx.loc;
        let cls = info_arg(args, 0)?;
        let range = *args.get(1).ok_or_else(MetaError::invalid)?;
        let block = self.inst.eval_loc.unwrap_or(loc);
        if !ctx.plainly {
            return fail(
                loc,
                "'std::meta::define_aggregate' can only be called from a 'consteval' block",
            );
        }
        let t = match self.ast.reflections.get(cls) {
            Reflectee::Type(t) => Some(self.ast.types.canonical(t)),
            _ => None,
        };
        let Some((t, r)) = t.and_then(|t| Some((t, self.ast.types.as_record(t)?))) else {
            return fail(loc, format!("'{}' is not a class type", self.reflection_str(cls)));
        };
        if t.quals_bits() != 0 {
            return fail(loc, format!("'{}' is a cv-qualified class type", self.ty_str(t)));
        }
        if self.defining.iter().any(|&d| self.same_entity(d, r)) {
            return fail(loc, format!("'{}' is being defined", self.ty_str(t)));
        }
        if let Some(def) = self.ast.definition(r) {
            let dloc = self.ast.decls.get(def).loc();
            return Err(vec![
                Note::new(loc, format!("'{}' is already complete", self.ty_str(t))),
                Note::new(dloc, "definition is here"),
            ]
            .into());
        }
        // The consteval block's scope is where the block is declared (the
        // bridge's context), not `ctx.scope`: that is the innermost
        // function on the stack (the block's own body, or a callee).
        let scope = self.ctx;
        let target = self.ast.decls.get(r).ctx().unwrap_or(self.ast.tu);
        if let Some(between) = self.intervening_scope(scope, target) {
            let what = self.scope_str(between);
            return Err(vec![
                Note::new(
                    loc,
                    format!(
                        "{what} intervenes between the scope of '{}' and the 'consteval' block",
                        self.ty_str(t)
                    ),
                ),
                Note::new(block, "'consteval' block defined here"),
            ]
            .into());
        }
        // One pass over the descriptions: kinds, complete types, names.
        let n = oven.meta_range_len(range);
        let mut specs = oven.take_dms_buf();
        let mut names: FxHashSet<Symbol> = FxHashSet::default();
        names.reserve(usize::try_from(n).unwrap_or(0).min(1 << 20));
        let underscore = self.ast.symbols().intern("_");
        let mut result = Ok(());
        for i in 0..n {
            match self.aggregate_element(oven, range, i, (r, underscore), &mut names, loc) {
                Ok(d) => specs.push(d),
                Err(e) => {
                    result = Err(e);
                    break;
                }
            }
        }
        if let Err(e) = result {
            oven.put_dms_buf(specs);
            return Err(e);
        }
        // `#pragma pack` where the block is does not apply.
        let pack = std::mem::take(&mut self.pack);
        let saved = self.ctx;
        self.act_on_start_record_definition(r);
        for d in &specs {
            let annots = oven.dms_annots(d.annots).to_vec();
            self.inject_field(r, d, &annots, loc, block);
        }
        self.lend_oven(oven, |s| s.act_on_finish_record_definition(r, false));
        self.ctx = saved;
        self.pack = pack;
        oven.put_dms_buf(specs);
        Ok(cls)
    }

    /// Element `i` of `define_aggregate`'s range: a description whose
    /// type is complete (or a reference) and whose name is not in
    /// `names` (`_` may repeat, [basic.scope.scope]/5).
    fn aggregate_element(
        &mut self,
        oven: &mut Oven,
        range: u128,
        i: u64,
        (r, underscore): (DeclId, Symbol),
        names: &mut FxHashSet<Symbol>,
        loc: Loc,
    ) -> Result<Dms, MetaError> {
        let e = oven.meta_range_at(range, i)?;
        let Reflectee::DataMemberSpec(id) = self.ast.reflections.get(e) else {
            return fail(
                loc,
                format!(
                    "element {i} ('{}') is not a data member description",
                    self.reflection_str(e)
                ),
            );
        };
        let d = *oven.dms(id).ok_or_else(MetaError::invalid)?;
        if !self.ast.types.is_reference(d.ty) {
            if self.undefined_spec(d.ty).is_some() {
                self.lend_oven(oven, |s| s.require_complete(d.ty, loc));
            }
            if !self.is_complete(d.ty)
                || self.ast.types.as_record(d.ty).is_some_and(|x| self.same_entity(x, r))
            {
                return fail(
                    loc,
                    format!(
                        "type '{}' of data member description {i} is incomplete",
                        self.ty_str(d.ty)
                    ),
                );
            }
            // [class.mem.general]: no member of abstract class type.
            if self.abstract_class(d.ty).is_some() {
                return fail(
                    loc,
                    format!(
                        "type '{}' of data member description {i} is an abstract class",
                        self.ty_str(d.ty)
                    ),
                );
            }
        }
        if let Some(n) = d.name
            && n != underscore
            && !names.insert(n)
        {
            let s = self.ast.symbols().as_str(n).to_owned();
            return fail(loc, format!("name '{s}' used in multiple data member descriptions"));
        }
        Ok(d)
    }

    /// The innermost function or class scope enclosing exactly one of
    /// `scope` (the consteval block's) and `target` (the class's target
    /// scope): below their nearest common enclosing scope.
    fn intervening_scope(&self, scope: DeclId, target: DeclId) -> Option<DeclId> {
        let chain = |mut d: DeclId| {
            let mut v = vec![d];
            while let Some(p) = self.ast.decls.get(d).ctx() {
                if v.len() > 4096 {
                    break;
                }
                v.push(p);
                d = p;
            }
            v
        };
        let (a, b) = (chain(scope), chain(target));
        let common = a.iter().find(|x| b.contains(x)).copied();
        let is_scope = |d: DeclId| {
            self.ast.decls.get(d).kind().is_function() || self.ast.decls.record(d).is_some()
        };
        // Both chains walk the same parent links, so a shared scope is the
        // same DeclId.
        let below = |v: &[DeclId]| {
            v.iter().take_while(|&&x| Some(x) != common).copied().find(|&x| is_scope(x)) // same_entity: raw ok
        };
        below(&a).or_else(|| below(&b))
    }

    /// Scope `d` named for a note: `'f'`, `'C'`, or `a lambda`.
    fn scope_str(&self, d: DeclId) -> String {
        let closure = |x: DeclId| self.ast.decls.record(x).is_some_and(|r| r.lambda.is_some());
        let lambda = closure(d)
            || (self.ast.decls.get(d).kind().is_function()
                && self.ast.decls.get(d).ctx().is_some_and(closure));
        if lambda { "a lambda".to_owned() } else { format!("'{}'", self.decl_name(d)) }
    }

    /// Appends the public data member `d` describes to class `r` (being
    /// defined) at `loc`, injected by the evaluation at `point`
    /// (spec/reflection.md §6.2).
    pub(super) fn inject_field(
        &mut self,
        r: DeclId,
        d: &Dms,
        annots: &[ReflectionId],
        loc: Loc,
        point: Loc,
    ) -> DeclId {
        let name = d.name.map_or(DeclName::EMPTY, DeclName::ident);
        let mut h = DeclHead::new(r, name, loc);
        h.flags = DeclFlags::INJECTED;
        let width = d
            .bit_width
            .map(|w| self.builder().int_literal(QualType::INT, w.into(), Radix::Decimal, loc));
        if let (Some(w), Some(n)) = (d.bit_width, d.name)
            && let Some((bits, _)) = crate::constfold::int_info(&self.ast, d.ty)
            && w > bits
            && self.ast.types.builtin(d.ty) != Some(BuiltinType::Bool)
        {
            // As for a declared bit-field (clang warns for all but `bool`).
            let msg = self
                .diag(DiagId::sema_bitfield_width_exceeds, loc)
                .arg(self.ast.symbols().as_str(n).to_owned())
                .arg(w.to_string())
                .arg(bits.to_string())
                .arg(if bits == 1 { "" } else { "s" });
            self.emit(msg);
        }
        let flags = if d.nua { FieldData::NO_UNIQUE_ADDRESS } else { 0 };
        let f = self.builder().field(h, d.ty, width, flags);
        self.ast.decls.set_access(f, Access::Public);
        if let Some(a) = d.alignment {
            let e = self.builder().int_literal(QualType::INT, a.into(), Radix::Decimal, loc);
            let _ = self.ast.decls.cold.alignment.insert(f, e);
        }
        let _ = self.ast.decls.cold.injection_point.insert(f, point);
        if !annots.is_empty() {
            self.inject_annotations(f, annots);
        }
        f
    }

    /// Attaches description annotations `annots` (constant reflections,
    /// [`Sema::dms_annotations`]) to injected member `f`. Like GCC 16.1's,
    /// they have no source location.
    fn inject_annotations(&mut self, f: DeclId, annots: &[ReflectionId]) {
        let mut ids = Vec::with_capacity(annots.len());
        for &c in annots {
            let value = match self.ast.reflections.get(c) {
                Reflectee::Value(cv) => Some(cv),
                _ => self.class_constant(c),
            };
            let Some(cv) = value else { continue };
            let ty = self.ast.types.canonical(self.ast.consts.ty(cv));
            let ty = crate::conv::strip_cv(self, ty);
            let expr = self.builder().int_literal(QualType::INT, 0, Radix::Decimal, Loc::INVALID);
            let a =
                Annotation { decl: f, value: Some(cv), loc: Loc::INVALID, written_type: ty, expr };
            ids.push(self.ast.annots.push(a));
        }
        let l = self.ast.lists.push(&ids);
        self.ast.annots.attach(f, l, &mut self.ast.lists);
    }
}
