//! The `AArch64` SVE ACLE sizeless builtin types (spec/types.md §5.1):
//! `__SVInt8_t` ... `__SVBool_t`, `__SVCount_t`.
//!
//! clang declares them as implicit translation-unit typedefs on `AArch64`
//! (`Sema::Initialize`, `AArch64SVEACLETypes.def`). They are complete
//! types without a size: `sizeof`/`alignof`, arrays, fields and variables
//! of static or thread storage duration are errors (clang
//! `err_sizeof_alignof_typeof_sizeless`, `err_array_incomplete_or_sizeless_type`,
//! `err_field_incomplete_or_sizeless`, `err_sizeless_nonlocal`). kiln has
//! no SVE target feature, so — as clang without `+sve` — every use inside
//! a function body is an error (clang `checkTypeSupport`,
//! `err_sve_vector_in_non_sve_target`): only declarations can name them,
//! and codegen lowers them to scalable vectors in declarations only.

use kiln_ast::decl::{DeclFlags, DeclKind, TlsKind, VarStorage};
use kiln_ast::names::DeclName;
use kiln_ast::types::BuiltinType;
use kiln_ast::{DeclHead, DeclId, QualType};
use kiln_diag::DiagId;
use kiln_source::Loc;

use crate::Sema;

impl Sema<'_, '_> {
    /// Declares the implicit typedefs naming the SVE builtins on `AArch64`.
    pub(crate) fn declare_sve_typedefs(&mut self) {
        if self.abi.triple.arch != kiln_abi::target::Arch::AArch64 {
            return;
        }
        let tu = self.ast.tu;
        for b in BuiltinType::ALL.into_iter().filter(|b| b.is_sizeless()) {
            let n = DeclName::ident(self.ast.symbols().intern(b.name()));
            let h = DeclHead::new(tu, n, Loc::INVALID).flags(DeclFlags::IMPLICIT);
            let _ = self.builder().typedef(h, DeclKind::Typedef, b.qual_type(), None);
        }
    }

    /// The SVE types a use of a value of type `q` needs (clang
    /// `checkTypeSupport`): `q` itself, or a function type's parameter
    /// types and then its return type.
    fn sve_types_of(&self, q: QualType) -> Vec<QualType> {
        let t = &self.ast.types;
        if t.is_sizeless(q) {
            return vec![q];
        }
        // The written (sugared) types when the node is a function type.
        let Some(p) = t.proto_of(q.ty()).or_else(|| t.proto(q)) else { return Vec::new() };
        t.params(p)
            .iter()
            .copied()
            .chain(std::iter::once(p.ret()))
            .filter(|&x| t.is_sizeless(x))
            .collect()
    }

    /// clang `err_sve_vector_in_non_sve_target` for each SVE type a use of
    /// type `q` at `loc` needs.
    pub(crate) fn check_sve_support(&mut self, q: QualType, loc: Loc) {
        for x in self.sve_types_of(q) {
            let msg = self.diag(DiagId::sema_sve_type_without_sve, loc).arg(self.ty_arg(x));
            self.emit(msg.allow_duplicate());
        }
    }

    /// A reference to `d`: its type must be supported by the target (also
    /// at namespace scope: `auto p = &f;`). clang 19 checks parameters and
    /// functions here (a local variable was diagnosed at its declaration),
    /// and not in an unevaluated operand.
    pub(crate) fn check_sve_use(&mut self, d: DeclId, loc: Loc) {
        if self.unevaluated > 0
            || !matches!(
                self.ast.decls.get(d).kind(),
                DeclKind::ParmVar | DeclKind::Function | DeclKind::CXXMethod
            )
        {
            return;
        }
        if let Some(t) = self.ast.decls.get(d).ty() {
            self.check_sve_support(t, loc);
        }
    }

    /// A variable `v` of sizeless type: an error unless it has automatic
    /// storage duration ([basic.stc]), and then one without SVE support.
    pub(crate) fn check_sizeless_var(&mut self, v: DeclId, local: bool) {
        let n = *self.ast.decls.get(v);
        let Some(ty) = n.ty().filter(|&t| self.ast.types.is_sizeless(t)) else { return };
        let spec = self.ast.decls.var(v).map(|x| x.spec).unwrap_or_default();
        let automatic = local
            && !matches!(spec.storage(), VarStorage::Static | VarStorage::Extern)
            && spec.tls() == TlsKind::None;
        if automatic {
            self.check_sve_support(ty, n.loc());
        } else {
            let msg = self.diag(DiagId::sema_nonlocal_sizeless, n.loc()).arg(self.ty_arg(ty));
            self.emit(msg);
            self.ast.decls.set_flags(v, DeclFlags::INVALID);
        }
    }
}
