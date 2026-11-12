//! Dependent specializations of the builtin templates
//! `__make_integer_seq` and `__type_pack_element` (spec/modules.md
//! §11.11, "Forms carried by WP-1344").
//!
//! Every unit declares both when its `Sema` is created
//! (`declare_builtin_templates`), so the image names the importer's: an
//! `Implicit` record of kind [`MAKE_INTEGER_SEQ_KIND`] or
//! [`TYPE_PACK_ELEMENT_KIND`] (unnamed, at global scope, like a builtin
//! function's stub), which a `Tst` type with no underlying type names
//! with its arguments as written. A non-dependent one (its computed
//! type under the sugar) stays refused.

use kiln_ast::DeclId;
use kiln_ast::decl::{DeclFlags, DeclKind};
use kiln_ast::names::DeclName;

use super::{DeclRec, NONE, Payload, W, Writer};
use crate::Sema;

/// The `Implicit` record kind of `__make_integer_seq`.
pub(super) const MAKE_INTEGER_SEQ_KIND: u8 = 8;
/// The `Implicit` record kind of `__type_pack_element`.
pub(super) const TYPE_PACK_ELEMENT_KIND: u8 = 9;

/// The builtin template of an `Implicit` record kind, by name.
fn name_of(kind: u8) -> Option<&'static str> {
    match kind {
        MAKE_INTEGER_SEQ_KIND => Some("__make_integer_seq"),
        TYPE_PACK_ELEMENT_KIND => Some("__type_pack_element"),
        _ => None,
    }
}

/// Whether `kind` is a builtin template's `Implicit` record kind.
pub(super) fn is_kind(kind: u8) -> bool {
    name_of(kind).is_some()
}

impl Writer<'_, '_, '_> {
    /// The stub record of builtin template `t`, written on first use;
    /// `None` when `t` is no builtin template.
    pub(super) fn builtin_template_rec(&mut self, t: DeclId) -> W<Option<u32>> {
        let ast = &self.s.ast;
        let n = ast.decls.get(t);
        if n.kind() != DeclKind::BuiltinTemplate {
            return Ok(None);
        }
        if let Some(&r) = self.recs.get(&t) {
            return Ok(Some(r));
        }
        let kind = match n.data() {
            crate::templ::builtins::MAKE_INTEGER_SEQ => MAKE_INTEGER_SEQ_KIND,
            crate::templ::builtins::TYPE_PACK_ELEMENT => TYPE_PACK_ELEMENT_KIND,
            _ => return Err("a builtin template the image does not carry"),
        };
        let rec = self.img.decls.len() as u32;
        self.img.decls.push(DeclRec {
            flags: 0,
            owner: 0,
            access: 0,
            name: NONE,
            ctx: NONE,
            lexical: NONE,
            loc: NONE,
            prev: NONE,
            payload: Payload::Implicit { kind },
        });
        self.recs.insert(t, rec);
        Ok(Some(rec))
    }
}

/// The importer's side.
impl Sema<'_, '_> {
    /// The importer's builtin template of stub kind `kind`.
    pub(super) fn image_builtin_template(&mut self, kind: u8) -> DeclId {
        let tu = self.ast.tu;
        let Some(name) = name_of(kind) else { return tu };
        let sym = self.intern(name);
        self.ast
            .lookup_local(tu, DeclName::ident(sym))
            .iter()
            .find(|&p| {
                let n = self.ast.decls.get(p);
                n.kind() == DeclKind::BuiltinTemplate && n.has(DeclFlags::IMPLICIT)
            })
            .unwrap_or(tu)
    }
}
