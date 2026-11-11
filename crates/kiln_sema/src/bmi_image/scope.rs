//! Scopes and names of the declaration image beyond named namespaces and
//! export blocks (spec/modules.md §11.11, "Scopes and reserved names",
//! WP-1278): inline and unnamed namespaces, linkage specifications, the
//! namespace `std` sema may already have declared, and reserved names.
//!
//! A linkage specification is an `Export` record (transparent, unnamed,
//! a context like an export block) listed in [`Scopes::links`] with its
//! language; an inline namespace is a `Namespace` record listed in
//! [`Scopes::inline`]. The importer builds a `LinkageSpec` or an inline
//! namespace for them. An unnamed namespace is a `Namespace` record with
//! no name; the importer extends the unnamed namespace its scope already
//! has ([namespace.unnamed]/1, as the parser does) or declares it with
//! its implicit using-directive, which the writer leaves out.
//!
//! Sema declares two kinds of global-scope names by itself, which the
//! name gate cannot see (§11.11 "ODR merging"): builtins (a lazily
//! declared `__builtin_*` function, a predeclared `__builtin_va_list`)
//! and the hidden namespace `std` around an implicit `std::align_val_t`.
//! A declaration at global scope with a builtin's name, or of a name sema
//! predeclares there, falls back. The namespace `std` is carried: the
//! importer extends its implicit one, if it has one, as the parser's
//! first `namespace std {` does, and the unit's `std::align_val_t`
//! redeclares the implicit enumeration there.

use kiln_ast::decl::{DeclFlags, DeclKind, LinkageLang, NamespaceData, ScopeData, builtin};
use kiln_ast::names::DeclName;
use kiln_ast::{DeclHead, DeclId};

use super::{Image, Payload, Rd, Res, W, Wr, Writer, bad};
use crate::Sema;

/// A linkage specification record: the `Export` record `rec`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) struct LinkRec {
    rec: u32,
    /// `LinkageLang` (1 C, 2 C++).
    lang: u8,
    /// Written without braces (`extern "C" int x;`).
    braceless: bool,
}

/// The scope section.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub(super) struct Scopes {
    /// Inline namespace records, ascending.
    inline: Vec<u32>,
    /// Linkage specification records, ascending.
    links: Vec<LinkRec>,
}

/// `d`'s name is declared at global namespace scope: its context is the
/// translation unit, seen through export blocks, linkage specifications,
/// unnamed namespaces and unscoped enumerations (the scopes the gate's
/// `gated_names` sees through).
fn global_scope(s: &Sema<'_, '_>, d: DeclId) -> bool {
    let ast = &s.ast;
    let mut c = ast.decls.get(d).ctx();
    for _ in 0..4096 {
        let Some(x) = c else { return false };
        let n = ast.decls.get(x);
        let through = match n.kind() {
            DeclKind::TranslationUnit => return true,
            DeclKind::Export | DeclKind::LinkageSpec => true,
            DeclKind::Namespace => n.name() == DeclName::EMPTY,
            DeclKind::Enum => ast.decls.enumeration(x).is_some_and(|e| !e.is_scoped()),
            _ => false,
        };
        if !through {
            return false;
        }
        c = n.ctx();
    }
    false
}

impl Writer<'_, '_, '_> {
    /// Checks identifier `name` of `d` against the names sema declares by
    /// itself.
    pub(super) fn check_name(&self, d: DeclId, name: &str) -> W<()> {
        let reserved = super::reserved(name);
        if !reserved && name != "main" {
            return Ok(());
        }
        if !global_scope(self.s, d) {
            return Ok(());
        }
        if name == "main" {
            return Err("a declaration named `main` at global scope");
        }
        if builtin::by_name(name).is_some() {
            return Err("a reserved name of a builtin at global scope");
        }
        let ast = &self.s.ast;
        let sym = ast.symbols().get(name);
        let predeclared = sym.is_some_and(|sym| {
            ast.lookup_local(ast.tu, DeclName::ident(sym))
                .iter()
                .any(|p| ast.decls.get(p).has(DeclFlags::IMPLICIT))
        });
        if predeclared {
            return Err("a reserved name sema predeclares at global scope");
        }
        Ok(())
    }

    /// `p`, the previous declaration of `d`, is one the importer's sema
    /// declares by itself and the reader finds again ([`Sema::image_prev`]):
    /// the implicit namespace `std` or its `std::align_val_t`.
    pub(super) fn implicit_prev(&self, d: DeclId, p: DeclId) -> bool {
        let ast = &self.s.ast;
        let Some(std) = self.s.global_ops.implicit_std else { return false };
        match ast.decls.get(d).kind() {
            DeclKind::Namespace => ast.first(p) == std,
            DeclKind::Enum => {
                let n = ast.decls.get(p);
                n.has(DeclFlags::IMPLICIT)
                    && n.ctx().is_some_and(|c| ast.first(c) == std)
                    && ast
                        .decls
                        .get(d)
                        .name()
                        .as_ident()
                        .is_some_and(|s| ast.symbols().as_str(s) == "align_val_t")
            }
            _ => false,
        }
    }

    /// The namespace flags of record `i`, namespace `d`.
    pub(super) fn namespace_flags(&mut self, i: u32, flags: u32) -> W<()> {
        if flags & !(NamespaceData::INLINE | NamespaceData::ANONYMOUS) != 0 {
            return Err("a namespace with flags the image does not carry");
        }
        if flags & NamespaceData::INLINE != 0 {
            self.img.scopes.inline.push(i);
        }
        Ok(())
    }

    /// The record of linkage specification `d`, record `i`.
    pub(super) fn linkage_record(&mut self, d: DeclId, i: u32) -> W<Payload> {
        let ast = &self.s.ast;
        let sd = ast.decls.scope(d).ok_or("a malformed linkage specification")?;
        let lang = sd.linkage_lang().ok_or("a malformed linkage specification")?;
        if lang == LinkageLang::C && !global_scope(self.s, d) {
            // [dcl.link]/6: a C function or variable in a namespace is the
            // same entity as one of its name at global scope, which the
            // gate does not cover.
            return Err("a C linkage specification in a namespace");
        }
        let braceless = sd.aux & ScopeData::LINKAGE_BRACELESS != 0;
        self.img.scopes.links.push(LinkRec { rec: i, lang: lang as u8, braceless });
        Ok(Payload::Export)
    }

    /// An implicit declaration the importer declares again itself: the
    /// using-directive that follows an unnamed namespace.
    pub(super) fn implicit_scope_decl(&self, d: DeclId) -> bool {
        let ast = &self.s.ast;
        ast.decls.get(d).kind() == DeclKind::UsingDirective
            && ast.using_target(d).is_some_and(|ns| {
                let n = ast.decls.get(ns);
                n.kind() == DeclKind::Namespace && n.name() == DeclName::EMPTY
            })
    }
}

pub(super) fn encode_scopes(w: &mut Wr, s: &Scopes) {
    w.len(s.inline.len());
    for &r in &s.inline {
        w.u32(r);
    }
    w.len(s.links.len());
    for l in &s.links {
        w.u32(l.rec);
        w.u8(l.lang);
        w.u8(u8::from(l.braceless));
    }
}

pub(super) fn decode_scopes(r: &mut Rd) -> Res<Scopes> {
    let mut s = Scopes::default();
    let n = r.count(4)?;
    s.inline = Vec::with_capacity(n);
    for _ in 0..n {
        s.inline.push(r.u32()?);
    }
    let n = r.count(6)?;
    s.links = Vec::with_capacity(n);
    for _ in 0..n {
        let rec = r.u32()?;
        let lang = r.u8()?;
        let braceless = match r.u8()? {
            0 => false,
            1 => true,
            _ => return bad("a bad linkage specification"),
        };
        s.links.push(LinkRec { rec, lang, braceless });
    }
    Ok(s)
}

pub(super) fn validate_scopes(img: &Image) -> Res<()> {
    let s = &img.scopes;
    let rec = |r: u32| img.decls.get(r as usize);
    if !s.inline.windows(2).all(|w| w[0] < w[1])
        || !s
            .inline
            .iter()
            .all(|&r| rec(r).is_some_and(|d| matches!(d.payload, Payload::Namespace)))
    {
        return bad("a bad inline namespace");
    }
    if !s.links.windows(2).all(|w| w[0].rec < w[1].rec)
        || !s.links.iter().all(|l| {
            matches!(l.lang, 1 | 2)
                && rec(l.rec).is_some_and(|d| matches!(d.payload, Payload::Export))
        })
    {
        return bad("a bad linkage specification");
    }
    Ok(())
}

impl Sema<'_, '_> {
    /// Builds namespace record `i` (header `h`, previous `prev`).
    pub(super) fn image_namespace(
        &mut self,
        img: &Image,
        i: u32,
        h: DeclHead,
        mut prev: Option<DeclId>,
    ) -> DeclId {
        let inline = img.scopes.inline.binary_search(&i).is_ok();
        let tu = self.ast.tu;
        let at_tu = || h.ctx == tu;
        let anon = h.name == DeclName::EMPTY;
        let key = self.ast.primary_context(self.ast.redecl_context(h.ctx));
        if prev.is_none() && anon {
            prev = self.anon_namespaces.get(&key).copied();
        }
        if prev.is_none()
            && at_tu()
            && h.name.as_ident().is_some_and(|s| self.ast.symbols().as_str(s) == "std")
        {
            prev = self.global_ops.implicit_std;
        }
        let inline = inline
            || prev.is_some_and(|p| {
                self.ast.decls.namespace(self.ast.first(p)).is_some_and(NamespaceData::is_inline)
            });
        let ns = self.builder().namespace(h, inline, prev);
        if anon {
            self.anon_namespaces.entry(key).or_insert(ns);
            if prev.is_none() {
                let _ = self.using_directive(ns, h.loc, DeclFlags::IMPLICIT);
            }
        }
        ns
    }

    /// The previous declaration of enumeration record `i` (header `h`)
    /// sema declared by itself: the implicit `std::align_val_t` in the
    /// implicit namespace `std` that `h.ctx` extends.
    pub(super) fn image_enum_prev(&self, h: &DeclHead) -> Option<DeclId> {
        let std = self.global_ops.implicit_std?;
        if self.ast.first(h.ctx) != std {
            return None;
        }
        let sym = h.name.as_ident().filter(|&s| self.ast.symbols().as_str(s) == "align_val_t")?;
        self.ast.lookup_local(std, DeclName::ident(sym)).iter().find(|&p| {
            let n = self.ast.decls.get(p);
            n.kind() == DeclKind::Enum && n.has(DeclFlags::IMPLICIT)
        })
    }

    /// Builds `Export` record `i` (context `ctx`, header `h`): a linkage
    /// specification when the scope section lists it.
    pub(super) fn image_scope(&mut self, img: &Image, i: u32, ctx: DeclId, h: DeclHead) -> DeclId {
        let Ok(k) = img.scopes.links.binary_search_by_key(&i, |l| l.rec) else {
            return self.builder().export_decl(ctx, h.loc);
        };
        let l = img.scopes.links[k];
        let lang = if l.lang == 1 { LinkageLang::C } else { LinkageLang::Cxx };
        let ls = self.builder().linkage_spec_head(DeclHead::new(ctx, DeclName::EMPTY, h.loc), lang);
        if l.braceless {
            self.ast.decls.set_linkage_braceless(ls);
        }
        ls
    }
}
