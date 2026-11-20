//! The sections of `#pragma clang section` (clang `setNonAliasAttributes`;
//! spec/codegen.md §8 "Sections"): the implicit
//! [`AttrKind::PRAGMA_CLANG_SECTION`] attributes sema gives a definition
//! (spec/sema.md §4.5). A variable definition's `bss`, `data`, `rodata`
//! and `relro` sections become the global's attributes `"bss-section"`,
//! ... (the object file writer places the variable in the one that fits
//! it); a function definition's `text` section is its section unless a
//! `section` attribute names one.

use kiln_ast::DeclId;
use kiln_ast::attr::{AttrKind, CLANG_SECTION_KINDS};
use kiln_ast::ids::{AttrId, RawId as _, StringLitId};
use kiln_llvm::Attr;
use kiln_llvm::{FuncId, GlobalId};

use crate::module::Cg;

/// The `b` word of a `text` section attribute.
const TEXT: u32 = 3;

impl Cg<'_> {
    /// The `#pragma clang section` attributes of declaration `d`: kind
    /// and section name.
    fn pragma_sections(&self, d: DeclId) -> impl Iterator<Item = (u32, &[u8])> {
        let ast = self.ast;
        ast.lists.iter::<AttrId>(ast.attrs.of_decl(d)).filter_map(move |a| {
            let n = ast.attrs.get(a);
            let lit = StringLitId::from_raw(n.a)?;
            (n.kind == AttrKind::PRAGMA_CLANG_SECTION).then(|| (n.b, ast.strings.bytes(lit)))
        })
    }

    /// The data sections of variable definition `d` on its global `g`.
    pub(crate) fn pragma_var_sections(&mut self, d: DeclId, g: GlobalId) {
        let attrs: Vec<Attr> = self
            .pragma_sections(d)
            .filter(|&(k, _)| k != TEXT)
            .filter_map(|(k, name)| {
                let kind = CLANG_SECTION_KINDS.get(k as usize)?;
                Some(Attr::Str(
                    format!("{kind}-section").into(),
                    String::from_utf8_lossy(name).into(),
                ))
            })
            .collect();
        if !attrs.is_empty() {
            let set = self.m.attr_set(&attrs);
            self.m.global_mut(g).attrs = set;
        }
    }

    /// The `text` section of function definition `d` on `f`, unless a
    /// `section` attribute placed it.
    pub(crate) fn pragma_text_section(&mut self, d: DeclId, f: FuncId) {
        if self.ast.section_of(d).is_some() || self.m.func(f).section.is_some() {
            return;
        }
        let Some(name) = self
            .pragma_sections(d)
            .find(|&(k, _)| k == TEXT)
            .map(|(_, n)| String::from_utf8_lossy(n).into_owned())
        else {
            return;
        };
        self.m.func_mut(f).section = Some(name.into());
    }
}
