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

use super::special::{DeletedAs, DiagSites, InitEntity, SpecialKind};
use crate::Sema;
use crate::conv::{self, ConvCtx};

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
        let built = self.construct_special_at(
            to,
            class,
            k,
            Some(e),
            ConstructionKind::Complete,
            at,
            InitEntity::Var,
            sites,
        );
        let Some(c) = built else {
            return Some(self.recover(to, &[e], at));
        };
        // A by-value argument is a temporary destroyed at the end of the
        // full-expression ([expr.call]/7, the Itanium ABI's caller-side
        // destruction).
        Some(if ctx == ConvCtx::Argument { self.maybe_bind_temporary(c) } else { c })
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
