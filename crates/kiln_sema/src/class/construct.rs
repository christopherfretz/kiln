//! Construction of class objects from expressions ([dcl.init.general]/16.6,
//! [class.copy.ctor]; spec/sema-class.md §5.2 and §5.6): copy and move
//! construction from a glvalue or prvalue of the same or a derived class.
//!
//! The constructor is chosen by the Q7 approximation of overload
//! resolution ([`Sema::select_special`]): the copy and move constructors
//! (and any other constructor whose first parameter is a reference to, or
//! is, the class) ranked by their reference bindings. A selection that
//! needs full overload resolution (WP-068a) reports `sema_unsupported`.

use kiln_ast::expr::{ConstructInfo, ConstructionKind, ExprFlags, ExprKind, ValueCategory};
use kiln_ast::{ExprId, QualType};
use kiln_source::Loc;

use super::ctor_select::InitStyle;
use super::special::{CtorCall, DeletedAs, DiagSites, InitEntity, Selected, SpecialKind};
use crate::Sema;
use crate::conv::{self, ConvCtx};

/// How a constructor call by [`Sema::construct_class`] is built and
/// diagnosed.
#[derive(Clone, Debug)]
pub(crate) struct ConstructSite {
    /// The `CXXConstructExpr`'s location (the variable's name, the
    /// type of `T(a, b)`, the argument of a conversion).
    pub loc: Loc,
    /// A `CXXTemporaryObjectExpr` ending here (`T(a, b)`, `T{...}`).
    pub temp_end: Option<Loc>,
    /// The `)` of a parenthesized initializer (the end of a
    /// `CXXParenListInitExpr`).
    pub rparen: Option<Loc>,
    /// Where the access and deletion errors go, and their form.
    pub sites: DiagSites,
}

impl ConstructSite {
    /// The construction site of a variable (or other entity) at `at`:
    /// clang reports the access and deletion errors there.
    pub(crate) fn at(at: Loc) -> Self {
        ConstructSite {
            loc: at,
            temp_end: None,
            rparen: None,
            sites: DiagSites { access: at, deleted: at, deleted_as: DeletedAs::Call },
        }
    }
}

impl Sema<'_, '_> {
    /// The initialization of an object of class type `to` from `e`, of
    /// the same class or a class derived from it ([dcl.init.general]/16.6:
    /// the constructors are considered; a prvalue of the same class
    /// initializes the object directly, /16.6.1). `None` when `e` is not
    /// of such a class. Errors are emitted here: a deleted constructor at
    /// `entity_at` for a variable (in the form `deleted_as`), an
    /// inaccessible one at the initializer of a copy-initialization and at
    /// `entity_at` for a direct-initialization; the result is then a
    /// recovery expression.
    pub(crate) fn construct_from_class(
        &mut self,
        e: ExprId,
        to: QualType,
        ctx: ConvCtx,
        entity_at: Loc,
        deleted_as: DeletedAs,
    ) -> Option<ExprId> {
        let target = self.ast.types.as_record(to)?;
        let source = self.ast.types.as_record(self.ety(e))?;
        self.instantiate_for_init(target, entity_at);
        self.instantiate_for_init(source, self.begin_loc(e));
        let class = self.ast.definition(target)?;
        let src = conv::bases::complete_class(self, source)?;
        if !conv::bases::same_class(self, src, class)
            && !conv::bases::is_derived_from(self, src, class)
        {
            return None;
        }
        let vk = self.ast.exprs.get(e).value_category();
        if vk == ValueCategory::PRValue && conv::bases::same_class(self, src, class) {
            return Some(e);
        }
        let k =
            if vk == ValueCategory::LValue { SpecialKind::CopyCtor } else { SpecialKind::MoveCtor };
        let at = self.begin_loc(e);
        let (variable, direct) = match ctx {
            ConvCtx::Initialization => (true, false),
            ConvCtx::DirectInitialization | ConvCtx::Cast | ConvCtx::Condition => (true, true),
            _ => (false, false),
        };
        let sites = DiagSites {
            access: if direct { entity_at } else { at },
            deleted: if variable { entity_at } else { at },
            deleted_as,
        };
        let (entity, ck) = self.take_ctor_entity();
        let built = self.construct_special_at(to, class, k, Some(e), ck, at, entity, sites);
        let Some(c) = built else {
            return Some(self.recover(to, &[e], at));
        };
        // A by-value argument is a temporary destroyed at the end of the
        // full-expression ([expr.call]/7, the Itanium ABI's caller-side
        // destruction).
        Some(if ctx == ConvCtx::Argument { self.maybe_bind_temporary(c) } else { c })
    }

    /// The initialization of an object of class type `ty` by a
    /// constructor called with `args` ([dcl.init.general]/16.6.2,
    /// [dcl.init.list]/3.7; spec/sema-class.md §5.2). `None` when `ty` is
    /// not a complete class (the caller diagnoses). Errors are emitted
    /// here and give a recovery expression.
    pub(crate) fn construct_class(
        &mut self,
        ty: QualType,
        args: &[ExprId],
        style: InitStyle,
        site: ConstructSite,
    ) -> Option<ExprId> {
        let target = self.ast.types.as_record(ty)?;
        self.instantiate_for_init(target, site.loc);
        let class = self.ast.definition(target)?;
        let class = conv::bases::complete_class(self, class)?;
        let (entity, ck) = self.take_ctor_entity();
        let loc = site.loc;
        if style.is_list() && self.has_init_list_ctor(class) {
            // todo(WP-067b follow-up): initializer-list constructors
            // ([dcl.init.list]/3.7, [over.match.list]/1.1).
            self.unsupported("initialization by an initializer-list constructor", loc);
            return Some(self.recover(ty, args, loc));
        }
        let sel = self.resolve_ctor(class, args, style);
        if sel == Selected::Missing
            && style == InitStyle::Direct
            && !args.is_empty()
            && self.is_aggregate(ty)
        {
            // [dcl.init.general]/16.6.2.2: no constructor is viable, so
            // the aggregate is initialized from the parenthesized list.
            let rparen = site.rparen.or(site.temp_end).unwrap_or(loc);
            return Some(self.paren_aggregate_init(ty, args, loc, rparen));
        }
        if let Selected::Found(f) | Selected::Deleted(f) = sel
            && let [arg] = args
            && self.kind_of(f) == kiln_ast::decl::DeclKind::CXXConversion
        {
            // CWG2327: the conversion function initializes the object.
            return Some(self.call_conversion_function(*arg, f, f, site.sites.access));
        }
        let call = CtorCall {
            info: ConstructInfo { kind: ck, list: style.is_list(), ..ConstructInfo::COMPLETE },
            value_init: style.is_list() && args.is_empty(),
            temp_end: site.temp_end,
            narrowing: style.is_list(),
            reject_explicit: style == InitStyle::CopyList,
            ..CtorCall::new(args)
        };
        let k = if args.is_empty() { SpecialKind::DefaultCtor } else { SpecialKind::CopyCtor };
        let built = self.use_ctor(sel, ty, class, k, call, loc, entity, site.sites);
        Some(built.unwrap_or_else(|| self.recover(ty, args, loc)))
    }

    /// The entity of the outermost constructor call being built: the
    /// subobject of a mem-initializer ([`Sema::ctor_entity`]), else a
    /// complete object.
    pub(crate) fn take_ctor_entity(&mut self) -> (InitEntity, ConstructionKind) {
        self.ctor_entity.take().unwrap_or((InitEntity::Var, ConstructionKind::Complete))
    }

    /// The value-initialization of an object of class type `ty`
    /// ([dcl.init.general]/9) by its default constructor, zero-initialized
    /// first unless that is user-provided: a `CXXConstructExpr` at `loc`.
    /// `None` when `ty` is not a complete class.
    pub(crate) fn value_init_class(&mut self, ty: QualType, loc: Loc) -> Option<ExprId> {
        let target = self.ast.types.as_record(ty)?;
        self.instantiate_for_init(target, loc);
        let class = self.ast.definition(target)?;
        let class = conv::bases::complete_class(self, class)?;
        let (entity, kind) = self.take_ctor_entity();
        let sel = self.resolve_ctor(class, &[], InitStyle::Direct);
        let call = CtorCall {
            value_init: true,
            info: ConstructInfo { kind, ..ConstructInfo::COMPLETE },
            ..CtorCall::new(&[])
        };
        let sites = ConstructSite::at(loc).sites;
        let built =
            self.use_ctor(sel, ty, class, SpecialKind::DefaultCtor, call, loc, entity, sites);
        Some(built.unwrap_or_else(|| self.recover(ty, &[], loc)))
    }

    /// The copy-initialization of an object of class type `to` from the
    /// non-class expression `e` by a converting constructor
    /// ([dcl.init.general]/16.6.3, [over.match.copy]): the call wrapped in
    /// an implicit `ConstructorConversion`; for an argument, a
    /// temporary destroyed at the end of the full-expression. Errors are
    /// reported at `at` (the variable, or `e`). `None` when `to` is not a
    /// complete class or `e` is of class type.
    pub(crate) fn convert_to_class(
        &mut self,
        e: ExprId,
        to: QualType,
        ctx: ConvCtx,
        at: Loc,
        deleted_as: DeletedAs,
    ) -> Option<ExprId> {
        if self.ast.types.as_record(self.ety(e)).is_some() {
            return None;
        }
        self.construct_converted(e, to, ctx, at, deleted_as)
    }

    /// [`Sema::convert_to_class`] for any source `e` (a class source of
    /// an unrelated class, when the user-defined conversion chose a
    /// converting constructor: [over.match.copy]/1.1).
    pub(crate) fn construct_converted(
        &mut self,
        e: ExprId,
        to: QualType,
        ctx: ConvCtx,
        at: Loc,
        deleted_as: DeletedAs,
    ) -> Option<ExprId> {
        let deleted_as = match deleted_as {
            DeletedAs::Param(n) => DeletedAs::Conversion(Some(n)),
            DeletedAs::Call => DeletedAs::Conversion(None),
            x => x,
        };
        let site = ConstructSite {
            loc: self.begin_loc(e),
            temp_end: None,
            rparen: None,
            sites: DiagSites { access: at, deleted: at, deleted_as },
        };
        let c = self.construct_class(to, &[e], InitStyle::Copy, site)?;
        if self.is_err_expr(c) {
            return Some(c);
        }
        let x = conv::raw_cast(
            self,
            kiln_ast::expr::CastKind::ConstructorConversion,
            c,
            to,
            ValueCategory::PRValue,
            None,
        );
        Some(if ctx == ConvCtx::Argument { self.maybe_bind_temporary(x) } else { x })
    }

    /// `T()` for a class `T` ([expr.type.conv]/2, [dcl.init.general]/9):
    /// a value-initialized temporary, a `CXXTemporaryObjectExpr` calling
    /// the default constructor, zero-initialized first unless that
    /// constructor is user-provided (or deleted). `loc` is the type,
    /// `rparen` the `)`. `None` when `ty` is not a complete class.
    pub(crate) fn value_init_class_temporary(
        &mut self,
        ty: QualType,
        loc: Loc,
        rparen: Loc,
    ) -> Option<ExprId> {
        let target = self.ast.types.as_record(ty)?;
        self.instantiate_for_init(target, loc);
        let class = self.ast.definition(target)?;
        let class = conv::bases::complete_class(self, class)?;
        let c = self.construct_special(
            ty,
            class,
            SpecialKind::DefaultCtor,
            None,
            ConstructionKind::Complete,
            loc,
            InitEntity::Var,
        );
        let Some(c) = c else { return Some(self.recover(ty, &[], loc)) };
        if self.ast.exprs.get(c).kind() != ExprKind::CXXConstructExpr {
            return Some(c);
        }
        let ctor = self.ast.exprs.constructor(c);
        let zeroing = !self.is_user_provided(ctor);
        let info = ConstructInfo { zeroing, ..ConstructInfo::COMPLETE };
        let t = self.builder().temporary_object(ty, ctor, &[], info, ExprFlags::NONE, loc, rparen);
        Some(self.maybe_bind_temporary(t))
    }
}
