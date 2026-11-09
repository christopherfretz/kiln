//! Member declaration rules of [class.static.data], [class.union] and
//! [class.bit] (WP-1015): clang's static-data-member reinitialization
//! check of `AddInitializerToDecl`, the union rules of
//! `ActOnFunctionDeclarator` and `CheckCompletedCXXClass`, and the
//! anonymous bit-field rule of `CheckFieldDecl` (spec/sema.md §4.4
//! "Union, bit-field and static data member rules").

use kiln_ast::DeclId;
use kiln_ast::decl::{DeclKind, FieldData};
use kiln_diag::DiagId;
use kiln_source::Loc;

use crate::Sema;
use crate::init::Initializer;

impl Sema<'_, '_> {
    /// [class.static.data]/4: when the in-class declaration of a static
    /// data member has an initializer, the namespace-scope definition
    /// has none. Reported at the new initializer (clang
    /// `err_static_data_member_reinitialization`, at its expression
    /// location: `(` of a parenthesized list, `{` of a braced one) with
    /// a note at the in-class one; true when it did (the initializer is
    /// then dropped). A constexpr or inline member's in-class
    /// declaration is a definition: that is a redefinition, reported by
    /// the redeclaration merge before this runs.
    pub(crate) fn static_member_reinit(&mut self, v: DeclId, init: &Initializer) -> bool {
        let n = *self.ast.decls.get(v);
        let Some(ctx) = n.ctx() else { return false };
        if self.kind_of(ctx) != DeclKind::CXXRecord || self.is_in_class_static(v) {
            return false;
        }
        let first = self.ast.first(v);
        if first == v || !self.is_in_class_static(first) {
            return false;
        }
        let Some(old) = self.ast.decls.var(first).and_then(|x| x.init) else { return false };
        let loc: Loc = match init {
            Initializer::Expr(e) => self.expr_loc(*e),
            Initializer::Paren(_, lp, _) => *lp,
            Initializer::List(l) => l.lbrace,
        };
        let name = self.decl_name_str(n.name());
        let note = self.diag(DiagId::note_init_previous, self.expr_loc(old));
        let m = self.diag(DiagId::sema_static_data_member_reinit, loc).arg(name).with_note(note);
        self.emit(m);
        true
    }

    /// [class.union.general]/6 (CWG1460): at most one variant member of
    /// a union has a default member initializer. Run before a member
    /// with one (at `loc`: its name, or the first initialized member of
    /// an anonymous member about to be added) joins class `ctx`: when
    /// `ctx` is a union that already has one, clang's
    /// `checkDuplicateDefaultInit` reports `err_init_multiple_union` at
    /// `loc` with a note at the first. Only for the written definition
    /// (an instantiation is not diagnosed again).
    pub(crate) fn check_union_default_inits(&mut self, ctx: DeclId, loc: Loc) {
        if !self.inst.stack.is_empty() || self.kind_of(ctx) != DeclKind::CXXRecord {
            return;
        }
        if !self.ast.decls.record(ctx).is_some_and(|x| x.tag == kiln_ast::decl::TagKind::Union) {
            return;
        }
        let Some(p) = self.first_default_init(ctx) else { return };
        let note = self.diag(DiagId::note_init_previous, self.ast.decls.get(p).loc());
        let m = self.diag(DiagId::sema_init_multiple_union, loc).with_note(note).allow_duplicate();
        self.emit(m);
    }

    /// The first member of class `r` with a default member initializer,
    /// looking into its anonymous members (clang's
    /// `findDefaultInitializer`, which walks the indirect fields too).
    pub(crate) fn first_default_init(&self, r: DeclId) -> Option<DeclId> {
        self.first_default_init_in(r, 0)
    }

    fn first_default_init_in(&self, r: DeclId, depth: u32) -> Option<DeclId> {
        if depth > 64 {
            return None;
        }
        for m in self.ast.decls.members(r) {
            let n = self.ast.decls.get(m);
            if n.kind() != DeclKind::Field {
                continue;
            }
            let fd = self.ast.decls.field(m);
            if fd.is_some_and(|f| f.flags & FieldData::HAS_DEFAULT_INIT != 0) {
                return Some(m);
            }
            if n.name().as_ident().is_none()
                && fd.is_some_and(|f| f.bit_width.is_none())
                && let Some(rec) = n.ty().and_then(|t| self.ast.types.as_record(t))
                && self.ast.decls.get(rec).name().as_ident().is_none()
                && let Some(x) = self.first_default_init_in(rec, depth + 1)
            {
                return Some(x);
            }
        }
        None
    }
}
