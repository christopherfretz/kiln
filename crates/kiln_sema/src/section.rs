//! Named sections (spec/sema.md §4.5 `section`; clang `Sema::UnifySection`):
//! every declaration a GNU `section("name")` attribute places in a section
//! gives that section flags — a function read and execute, a variable read,
//! and write too unless it is constant storage — and a declaration whose
//! flags differ from the section's first declaration's is an error
//! ("'b' causes a section type conflict with 'a'"), with a note at that
//! first declaration. A function is placed when its declaration's
//! attributes are checked, a variable when its definition is complete;
//! neither inside a template instantiation.
//!
//! `#pragma clang section kind="name"` (clang `ActOnPragmaClangSection`,
//! spec/sema.md §4.5) places section `name` with the kind's flags at the
//! pragma and makes it the kind's current section (`""` clears the kind).
//! While one is current, every variable definition with static or thread
//! storage duration and no `section` attribute gets the current `bss`,
//! `data`, `rodata` and `relro` sections as implicit attributes (clang
//! `FinalizeDeclaration`; the object file writer picks the one that fits
//! the variable), and every function definition the `text` one (clang
//! `ActOnFunctionDeclarator`), [`AttrKind::PRAGMA_CLANG_SECTION`]. A
//! template's instantiations take the attributes of the template.

use kiln_ast::DeclId;
use kiln_ast::attr::{AttrKind, AttrNode, AttrSyntax};
use kiln_ast::decl::{DeclFlags, DefinitionBits, VarStorage};
use kiln_ast::ids::{AttrId, RawId as _, StringLitId};
use kiln_diag::DiagId;
use kiln_source::Loc;

use crate::Sema;

/// clang `ASTContext::PSF_Read`.
const READ: u8 = 1;
/// clang `ASTContext::PSF_Write`.
const WRITE: u8 = 2;
/// clang `ASTContext::PSF_Execute`.
const EXECUTE: u8 = 4;
/// clang `ASTContext::PSF_ZeroInit`.
const ZERO_INIT: u8 = 0x10;

/// A section placed so far (clang `ASTContext::SectionInfo`): the first
/// declaration or `#pragma clang section` that placed it, and its flags.
#[derive(Clone, Copy, Debug, PartialEq, Eq, kiln_snap::Snap)]
pub(crate) struct SectionInfo {
    /// The declaration (none for a pragma).
    decl: Option<DeclId>,
    /// The kind token of the pragma (none for a declaration).
    pragma: Option<Loc>,
    flags: u8,
}

/// A `#pragma clang section` kind (clang `PragmaClangSectionKind`), in
/// the order of [`kiln_ast::attr::CLANG_SECTION_KINDS`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ClangSectionKind {
    /// `bss`.
    Bss,
    /// `data`.
    Data,
    /// `rodata`.
    Rodata,
    /// `text`.
    Text,
    /// `relro`.
    Relro,
}

impl ClangSectionKind {
    /// The kind spelled `s`.
    pub fn from_name(s: &str) -> Option<Self> {
        Some(match s {
            "bss" => ClangSectionKind::Bss,
            "data" => ClangSectionKind::Data,
            "rodata" => ClangSectionKind::Rodata,
            "text" => ClangSectionKind::Text,
            "relro" => ClangSectionKind::Relro,
            _ => return None,
        })
    }

    /// Its spelling.
    pub fn name(self) -> &'static str {
        kiln_ast::attr::CLANG_SECTION_KINDS[self as usize]
    }
}

/// The current section of each `#pragma clang section` kind (clang's
/// `PragmaClang*Section`), by [`ClangSectionKind`]: the name and the
/// pragma's kind token.
#[derive(Clone, Debug, Default, PartialEq, Eq, kiln_snap::Snap)]
pub(crate) struct ClangSections([Option<(StringLitId, Loc)>; 5]);

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

    /// `#pragma clang section <kind>="<name>"` at the kind token `loc`
    /// (`name` empty: `""`, which clears the kind); clang
    /// `ActOnPragmaClangSection`. On ELF every name is a valid section
    /// (clang's `isValidSectionSpecifier` checks Mach-O only).
    pub fn act_on_pragma_clang_section(&mut self, kind: ClangSectionKind, loc: Loc, name: &[u8]) {
        let slot = kind as usize;
        if name.is_empty() {
            self.clang_sections.0[slot] = None;
            return;
        }
        let flags = match kind {
            ClangSectionKind::Bss => READ | WRITE | ZERO_INIT,
            ClangSectionKind::Data => READ | WRITE,
            ClangSectionKind::Rodata | ClangSectionKind::Relro => READ,
            ClangSectionKind::Text => READ | EXECUTE,
        };
        // clang `UnifySection(SectionName, SectionFlags, PragmaLoc)`.
        if let Some(prev) = self.sections.get(name).copied() {
            if prev.flags != flags {
                let mut msg = self
                    .diag(DiagId::sema_section_conflict, loc)
                    .arg("this")
                    .arg(self.section_arg(prev));
                if let Some(d) = prev.decl {
                    msg = msg.with_note(
                        self.diag(DiagId::note_declared_at, self.ast.decls.get(d).loc()),
                    );
                }
                if let Some(p) = prev.pragma {
                    msg = msg.with_note(self.diag(DiagId::note_pragma_entered_here, p));
                }
                self.emit(msg);
                return;
            }
        } else {
            self.sections.insert(name.into(), SectionInfo { decl: None, pragma: Some(loc), flags });
        }
        let lit = self.ast.strings.intern(kiln_ast::strings::StringEncoding::Ordinary, name);
        self.clang_sections.0[slot] = Some((lit, loc));
    }

    /// clang `FinalizeDeclaration`: variable `v`, just declared, takes
    /// the current `#pragma clang section` data sections when it is a
    /// definition with static or thread storage duration without a
    /// `section` attribute, outside a template instantiation.
    pub(crate) fn clang_section_var(&mut self, v: DeclId) {
        if self.clang_sections.0.iter().all(Option::is_none)
            || self.inst.stack.in_instantiation_record()
            || self.ast.section_of(v).is_some()
        {
            return;
        }
        let n = self.ast.decls.get(v);
        let ctx = n.ctx();
        let Some(var) = self.ast.decls.var(v) else { return };
        let global = !ctx.is_some_and(|c| self.ast.decls.get(c).kind().is_function())
            || matches!(var.spec.storage(), VarStorage::Static | VarStorage::Extern)
            || var.spec.tls() != kiln_ast::decl::TlsKind::None;
        if !global || !self.is_var_definition(v) {
            return;
        }
        let kinds = [
            ClangSectionKind::Bss,
            ClangSectionKind::Data,
            ClangSectionKind::Rodata,
            ClangSectionKind::Relro,
        ];
        let attrs: Vec<AttrId> = kinds
            .iter()
            .filter_map(|&k| self.clang_sections.0[k as usize].map(|s| (k, s)))
            .map(|(k, (lit, loc))| self.ast.attrs.push(clang_section_attr(k, lit, loc)))
            .collect();
        self.builder().append_attrs(v, &attrs);
    }

    /// clang `ActOnFunctionDeclarator` on a function definition `f`
    /// (`{`, `= default` or `= delete` follows the declarator): it takes
    /// the current `#pragma clang section text` section unless it has a
    /// `section` attribute.
    pub fn act_on_function_definition_declarator(&mut self, f: DeclId) {
        let Some((lit, loc)) = self.clang_sections.0[ClangSectionKind::Text as usize] else {
            return;
        };
        if self.inst.stack.in_instantiation_record()
            || !self.ast.decls.get(f).kind().is_function()
            || self.ast.section_of(f).is_some()
        {
            return;
        }
        let a = self.ast.attrs.push(clang_section_attr(ClangSectionKind::Text, lit, loc));
        self.builder().append_attrs(f, &[a]);
    }

    /// The `#pragma clang section` attributes of `from` on `to`, an
    /// instantiation of it (a kind `to` has already is kept).
    pub(crate) fn clone_clang_sections(&mut self, to: DeclId, from: DeclId) {
        let ast = &self.ast;
        let kinds = |d: DeclId| {
            ast.lists.iter::<AttrId>(ast.attrs.of_decl(d)).filter_map(|a| {
                let n = ast.attrs.get(a);
                (n.kind == AttrKind::PRAGMA_CLANG_SECTION).then_some((a, n.b))
            })
        };
        let have: Vec<u32> = kinds(to).map(|(_, k)| k).collect();
        let attrs: Vec<AttrId> =
            kinds(from).filter(|(_, k)| !have.contains(k)).map(|(a, _)| a).collect();
        if !attrs.is_empty() {
            self.builder().append_attrs(to, &attrs);
        }
    }

    /// The second argument of a section conflict with `prev` (clang's
    /// `operator<<` of a `SectionInfo`).
    fn section_arg(&self, prev: SectionInfo) -> String {
        match prev.decl {
            Some(d) => self.decl_arg(d),
            None => "a prior #pragma section".to_owned(),
        }
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
        let Some(&prev) = self.sections.get(&name) else {
            self.sections.insert(name, SectionInfo { decl: Some(d), pragma: None, flags });
            return;
        };
        if prev.flags == flags {
            return;
        }
        let mut msg = self
            .diag(DiagId::sema_section_conflict, self.ast.decls.get(d).loc())
            .arg(self.decl_arg(d))
            .arg(self.section_arg(prev));
        if let Some(first) = prev.decl {
            msg =
                msg.with_note(self.diag(DiagId::note_declared_at, self.ast.decls.get(first).loc()));
        }
        if let Some(p) = prev.pragma {
            msg = msg.with_note(self.diag(DiagId::note_pragma_entered_here, p));
        }
        self.emit(msg);
    }
}

/// The implicit attribute of section `lit` of kind `k`, entered at `loc`.
fn clang_section_attr(k: ClangSectionKind, lit: StringLitId, loc: Loc) -> AttrNode {
    AttrNode {
        kind: AttrKind::PRAGMA_CLANG_SECTION,
        syntax: AttrSyntax::Gnu,
        flags: AttrNode::IMPLICIT,
        loc,
        a: lit.raw(),
        b: k as u32,
    }
}
