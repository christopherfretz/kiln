//! Named sections (spec/sema.md §4.5 `section`; clang `Sema::UnifySection`):
//! every declaration a GNU `section("name")` attribute places in a section
//! gives that section flags — a function read and execute, a variable read,
//! and write too unless it is constant storage — and a declaration whose
//! flags differ from the section's first declaration's is an error
//! ("'b' causes a section type conflict with 'a'"), with a note at that
//! first declaration. A function is placed when its declaration's
//! attributes are checked, a variable when its definition is complete;
//! neither inside a template instantiation.

use kiln_ast::DeclId;
use kiln_ast::attr::AttrKind;
use kiln_ast::decl::{DeclFlags, DefinitionBits, VarStorage};
use kiln_ast::ids::{AttrId, RawId as _, StringLitId};
use kiln_diag::DiagId;

use crate::Sema;

/// clang `ASTContext::PSF_Read`.
const READ: u8 = 1;
/// clang `ASTContext::PSF_Write`.
const WRITE: u8 = 2;
/// clang `ASTContext::PSF_Execute`.
const EXECUTE: u8 = 4;

impl Sema<'_, '_> {
    /// clang `handleSectionAttr`: function `d` declared with the checked
    /// attributes `attrs` is placed in its section, read and execute.
    pub(crate) fn unify_fn_section(&mut self, d: DeclId, attrs: &[AttrId]) {
        if !self.ast.decls.get(d).kind().is_function() || self.inst.stack.in_instantiation_record()
        {
            return;
        }
        let Some(lit) = attrs.iter().find_map(|&a| {
            let n = self.ast.attrs.get(a);
            (n.kind == AttrKind::SECTION).then(|| StringLitId::from_raw(n.a)).flatten()
        }) else {
            return;
        };
        self.unify_section(lit, READ | EXECUTE, d);
    }

    /// clang `CheckCompleteVariableDeclaration`: the definition of
    /// variable `v` with static or thread storage duration is placed in
    /// its section, read only when its type is constant storage (`const`
    /// or a reference, no `mutable` member, a trivial destructor) and its
    /// initialization a constant initialization ([basic.start.static]/2),
    /// read and write otherwise.
    pub(crate) fn unify_var_section(&mut self, v: DeclId) {
        let n = self.ast.decls.get(v);
        if n.has(DeclFlags::INVALID) || self.inst.stack.in_instantiation_record() {
            return;
        }
        let (ctx, ty) = (n.ctx(), n.ty().unwrap_or(kiln_ast::QualType::ERROR));
        // Inherited from an earlier declaration too (clang `getAttr`).
        let Some(lit) = self.ast.section_of(v) else { return };
        let Some(var) = self.ast.decls.var(v) else { return };
        let init = var.init;
        let global = !ctx.is_some_and(|c| self.ast.decls.get(c).kind().is_function())
            || matches!(var.spec.storage(), VarStorage::Static | VarStorage::Extern)
            || var.spec.tls() != kiln_ast::decl::TlsKind::None;
        if !global || !self.is_var_definition(v) {
            return;
        }
        let flags = if self.constant_storage(ty) {
            // A dependent initializer is not checked (clang keeps
            // `HasConstInit`).
            let constant = match init {
                Some(e)
                    if !self.ast.types.is_dependent(ty)
                        && !self.ast.exprs.get(e).is_value_dependent() =>
                {
                    self.is_constant_initialized(v)
                }
                _ => true,
            };
            if constant { READ } else { READ | WRITE }
        } else {
            READ | WRITE
        };
        self.unify_section(lit, flags, v);
    }

    /// clang `QualType::isNonConstantStorage(Ctx, ExcludeCtor=true,
    /// ExcludeDtor=false)` is empty for `ty`.
    fn constant_storage(&self, ty: kiln_ast::QualType) -> bool {
        if self.ast.types.is_reference(ty) {
            return true;
        }
        let ty = self.ast.types.canonical(ty);
        let base = self.base_element(ty);
        if !base.quals().is_const() && !ty.quals().is_const() {
            return false;
        }
        let Some(r) = self.ast.types.as_record(base) else { return true };
        !self.defn_bits(r).contains(DefinitionBits::HAS_MUTABLE_FIELDS)
            && self.ast.destructor(r).is_none_or(|d| self.fn_spec(d).is_trivial())
    }

    /// clang `Sema::UnifySection`: `d` with `flags` joins section `lit`;
    /// the first declaration in a section fixes its flags.
    fn unify_section(&mut self, lit: StringLitId, flags: u8, d: DeclId) {
        let name: Box<[u8]> = self.ast.strings.bytes(lit).into();
        let Some(&(first, prev)) = self.sections.get(&name) else {
            self.sections.insert(name, (d, flags));
            return;
        };
        if prev == flags {
            return;
        }
        let first_loc = self.ast.decls.get(first).loc();
        let msg = self
            .diag(DiagId::sema_section_conflict, self.ast.decls.get(d).loc())
            .arg(self.decl_arg(d))
            .arg(self.decl_arg(first))
            .with_note(self.diag(DiagId::note_declared_at, first_loc));
        self.emit(msg);
    }
}
