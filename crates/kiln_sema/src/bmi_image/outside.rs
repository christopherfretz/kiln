//! What a body or expression names when it names a declaration that has
//! no record (spec/modules.md §11.11, "Forms carried by WP-1360").
//!
//! "a body referring to a declaration outside the image" is a bucket:
//! [`Writer::outside_why`] names the kind of declaration in the reason,
//! so the census counts each kind apart. One kind is carried rather than
//! refused: a `noexcept` operand naming a member its class declares
//! later ([class.mem.general]/7: the operand is a complete-class context,
//! [`TEXPR_LATER`]) goes through the [`super::exprs::LateNx`] path as one
//! naming the function's parameters does.

use kiln_ast::DeclId;
use kiln_ast::decl::{DeclFlags, DeclKind};

use kiln_ast::decl::Defaulted;

use super::{DECL_FLAGS, DEP, DeclRec, NONE, Payload, Writer};

/// The target is a compiler builtin sema declared lazily and that has no
/// record yet (`__builtin_expect`, ...).
pub(super) const OUT_BUILTIN: &str =
    "a body referring to a declaration outside the image (a compiler builtin)";
/// An implicit function sema declared that is no builtin.
pub(super) const OUT_IMPLICIT_FN: &str =
    "a body referring to a declaration outside the image (an implicitly declared function)";
/// An implicit special member of a class.
pub(super) const OUT_IMPLICIT_MEMBER: &str =
    "a body referring to a declaration outside the image (an implicit member)";
/// A predefined or other implicit entity (`__func__`, an implicit
/// variable or type).
pub(super) const OUT_PREDEFINED: &str =
    "a body referring to a declaration outside the image (a predefined entity)";
/// A declaration of another unit (an imported module or header unit).
pub(super) const OUT_OTHER_UNIT: &str =
    "a body referring to a declaration outside the image (a declaration of another unit)";
/// A declaration the census refused already (a cascade).
pub(super) const OUT_REFUSED: &str =
    "a body referring to a declaration outside the image (a refused declaration)";
/// A member of a class being recorded that the class declares later.
pub(super) const OUT_LATER_MEMBER: &str =
    "a body referring to a declaration outside the image (a later member of its class)";
/// A declaration of the unit recorded later or never.
pub(super) const OUT_OTHER: &str =
    "a body referring to a declaration outside the image (a declaration not recorded)";

/// A declaration whose record is not of the kind the use needs (a
/// variable named as a function, ...).
pub(super) const OUT_KIND: &str =
    "a body referring to a declaration outside the image (a record of another kind)";
/// An overloaded member access of a dependent call (`operator()(r.begin(),
/// r.end())` in a member template, [over.call.func]/3): sema keeps the set
/// in a side table (`ovl.members`) the image does not carry, so the
/// importer's access would name only the first candidate. Its own reason
/// rather than "outside the image": the candidates may all have records.
pub(super) const OUT_MEMBER_OVERLOADS: &str = "a template expression (an overloaded member access)";
/// A base-specifier of a class with no record (an inherited member's
/// path).
pub(super) const OUT_BASE: &str =
    "a body referring to a declaration outside the image (a base of a class outside the image)";
/// A local declaration (or the declaration a temporary extends) whose
/// function has no record.
pub(super) const OUT_LOCAL: &str =
    "a body referring to a declaration outside the image (a local declaration of another function)";
/// A requires-expression parameter its requires-expression did not
/// declare.
pub(super) const OUT_REQ_PARAM: &str = "a body referring to a declaration outside the image (a parameter of another requires-expression)";
/// A function explicit specialization the image does not carry (its own
/// reason, WP-1352).
pub(super) const OUT_EXPLICIT_SPEC: &str =
    "a body referring to a declaration outside the image (an explicit specialization)";

/// An expression in a type naming a member its class declares after it
/// (a `noexcept` operand, a complete-class context).
pub(super) const TEXPR_LATER: &str = "an expression in a type naming a later class member";

/// "a type declared outside the image", by the kind of declaration the
/// type names (WP-1360).
pub(super) const TY_PREDEFINED: &str = "a type declared outside the image (a predefined type)";
pub(super) const TY_OTHER_UNIT: &str =
    "a type declared outside the image (a declaration of another unit)";
pub(super) const TY_REFUSED: &str = "a type declared outside the image (a refused declaration)";
pub(super) const TY_LAMBDA: &str = "a type declared outside the image (a closure type)";
pub(super) const TY_LOCAL: &str =
    "a type declared outside the image (a local class or enumeration)";
pub(super) const TY_LATER: &str =
    "a type declared outside the image (a class named before its record)";

impl Writer<'_, '_, '_> {
    /// The reason for a type naming `d`, which has no record.
    pub(super) fn outside_type_why(&self, d: DeclId) -> &'static str {
        let ast = &self.s.ast;
        let n = ast.decls.get(d);
        if ast.modules.owner(d).unit != 0 {
            TY_OTHER_UNIT
        } else if ast
            .decls
            .record(d)
            .is_some_and(|r| r.defn.contains(kiln_ast::decl::DefinitionBits::LAMBDA))
        {
            TY_LAMBDA
        } else if n.has(DeclFlags::IMPLICIT) {
            TY_PREDEFINED
        } else if self.census_refused(d) {
            TY_REFUSED
        } else if self.local(d) {
            TY_LOCAL
        } else {
            TY_LATER
        }
    }

    /// The reason for a body naming `d`, which has no record.
    pub(super) fn outside_why(&self, d: DeclId) -> &'static str {
        let ast = &self.s.ast;
        let n = ast.decls.get(d);
        if ast.modules.owner(d).unit != 0 {
            return OUT_OTHER_UNIT;
        }
        if n.has(DeclFlags::IMPLICIT) {
            let in_class = n.ctx().is_some_and(|c| ast.decls.get(c).kind() == DeclKind::CXXRecord);
            return match n.kind() {
                DeclKind::Function if ast.decls.builtin_id(d).is_some() => OUT_BUILTIN,
                k if in_class
                    && matches!(
                        k,
                        DeclKind::CXXConstructor
                            | DeclKind::CXXDestructor
                            | DeclKind::CXXMethod
                            | DeclKind::CXXConversion
                    ) =>
                {
                    OUT_IMPLICIT_MEMBER
                }
                DeclKind::Function => OUT_IMPLICIT_FN,
                _ => OUT_PREDEFINED,
            };
        }
        if self.census_refused(d) {
            return OUT_REFUSED;
        }
        if self.later_member(d) {
            return OUT_LATER_MEMBER;
        }
        OUT_OTHER
    }

    /// The function template whose pattern `f` is, when its type was set
    /// from a redeclaration's deduced return type (`set_deduced_return`
    /// gives every redeclaration the deducing definition's type).
    pub(super) fn deduced_from_redecl(&self, f: DeclId) -> Option<DeclId> {
        let ast = &self.s.ast;
        if !self.s.declared_returns.contains_key(&ast.first(f)) {
            return None;
        }
        ast.described_template(f)
    }

    /// WP-1360: the record standing for template type parameter `p` of a
    /// redeclaration of [`Writer::redecl_ttps`]'s template, which has no
    /// record yet: that template's own parameter of the same depth and
    /// index ([temp.over.link]/6: equivalent parameters of equivalent
    /// declarations).
    pub(super) fn redecl_ttp(&self, p: DeclId) -> Option<u32> {
        let ast = &self.s.ast;
        let t = self.redecl_ttps?;
        let info = ast.template_parm(p)?;
        let own = |tm: DeclId| {
            let ti = ast.template_info(tm)?;
            let list = ast.templates.param_list(ti.params);
            Some(ast.lists.iter::<DeclId>(list.params).collect::<Vec<_>>())
        };
        // `p` is a parameter of another declaration of the template.
        let pattern = ast.template_info(t)?.pattern?;
        let other = ast
            .redecls_of(pattern)
            .filter_map(|f| ast.described_template(f))
            .any(|tm| tm != t && own(tm).is_some_and(|ps| ps.contains(&p)));
        if !other {
            return None;
        }
        let mine = own(t)?.into_iter().find(|&q| {
            ast.template_parm(q).is_some_and(|x| {
                x.depth == info.depth && x.index == info.index && x.pack == info.pack
            }) && ast.decls.get(q).kind() == ast.decls.get(p).kind()
        })?;
        self.recs.get(&mine).copied()
    }

    /// The record of `d` when it is an implicitly declared special member
    /// of a closed class whose record exists, written now (WP-1360): a
    /// type of a later declaration names it (`noexcept(C())`, `->
    /// decltype(C()(...))`), before the walk's implicit records, which
    /// come after every declaration. Sema declared it lazily at that use
    /// ([class.default.ctor]/1, [class.copy.ctor]/6), as the importer
    /// does at the record.
    pub(super) fn implicit_member_now(&mut self, d: DeclId) -> Option<u32> {
        let s = self.s;
        let ast = &s.ast;
        let n = ast.decls.get(d);
        if !n.has(DeclFlags::IMPLICIT)
            || n.flags().bits() & !DECL_FLAGS != DeclFlags::IMPLICIT.bits()
        {
            return None;
        }
        let k = s.special_kind_of(d)?;
        let implicit_default =
            ast.decls.function(d).is_some_and(|f| f.spec.defaulted() == Defaulted::Implicit);
        let c = n.ctx().and_then(|c| self.class_rec(c))?;
        let closed = matches!(&self.img.decls[c as usize],
            DeclRec { flags, payload: Payload::Record { def: Some(df), .. }, .. }
                if df.end != NONE && flags & DEP == 0);
        if !implicit_default || !closed {
            return None;
        }
        let o = ast.modules.owner(d);
        let i = u32::try_from(self.img.decls.len()).ok()?;
        self.img.decls.push(DeclRec {
            flags: n.flags().bits() & DECL_FLAGS,
            owner: u8::from(o.purview) | (u8::from(o.exported) << 1),
            access: 0,
            name: NONE,
            ctx: c,
            lexical: NONE,
            loc: NONE,
            prev: NONE,
            payload: Payload::Implicit { kind: k as u8 },
        });
        self.recs.insert(d, i);
        Some(i)
    }

    /// `d` is a member of a class whose record exists (being recorded)
    /// while `d` has none yet.
    pub(super) fn later_member(&self, d: DeclId) -> bool {
        let ast = &self.s.ast;
        let n = ast.decls.get(d);
        !self.recs.contains_key(&d)
            && n.ctx().is_some_and(|c| {
                ast.decls.get(c).kind() == DeclKind::CXXRecord && self.recs.contains_key(&c)
            })
    }
}
