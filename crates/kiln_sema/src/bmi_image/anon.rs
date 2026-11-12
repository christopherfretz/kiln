//! Anonymous unions and structs in a class (spec/modules.md §11.11,
//! "Forms carried by WP-1345").
//!
//! `union { char c; T v; };` in a class ([class.union.anon]) is an
//! unnamed class definition (a `Record` record whose definition keeps
//! its `ANONYMOUS` bit), then what sema's `act_on_anonymous_record`
//! declares for it: the implicit unnamed member object
//! ([`Payload::AnonObj`], kind 30, naming the class record) and, for
//! each named member of the class (or of an anonymous class nested in
//! it), an `IndirectField` injected into the enclosing class
//! ([`Payload::Indirect`], kind 31, naming the object record; its name
//! is the member's). The importer declares the object as the parser
//! does, through `act_on_decl_spec_only`, which injects the names, and
//! each `Indirect` record takes the injected member of its name. A
//! class template's pattern is carried the same way; its
//! instantiations redo the declaration (`subst/anon.rs`).
//!
//! Not carried: an anonymous union at namespace or block scope.

use kiln_ast::decl::{DeclFlags, DeclKind, DefinitionBits, TagKind};
use kiln_ast::names::DeclName;
use kiln_ast::types::TagKeyword;
use kiln_ast::{DeclHead, DeclId};

use super::{Image, NONE, Payload, W, Writer};
use crate::Sema;
use crate::declspec::DeclSpec;

/// The census reason of the forms this module leaves out.
const ANON: &str = "an anonymous union or struct member";

/// The `ANONYMOUS` bit of a carried class definition.
pub(super) const ANON_BIT: u64 = DefinitionBits::ANONYMOUS.bits();

impl Writer<'_, '_, '_> {
    /// Whether class `d` is an anonymous union or struct the image
    /// carries: a member of a class.
    pub(super) fn anon_class_ok(&self, d: DeclId) -> bool {
        let ast = &self.s.ast;
        let n = ast.decls.get(d);
        n.name() == DeclName::EMPTY
            && n.ctx().is_some_and(|c| ast.decls.get(c).kind() == DeclKind::CXXRecord)
    }

    /// The payload of `d` when it is what an anonymous class declared:
    /// its member object or an injected member.
    pub(super) fn anon_member(&mut self, d: DeclId) -> W<Option<Payload>> {
        let ast = &self.s.ast;
        let n = ast.decls.get(d);
        match n.kind() {
            DeclKind::Field
                if n.has(DeclFlags::IMPLICIT)
                    && n.name() == DeclName::EMPTY
                    && !self.capture_field(d) =>
            {
                let rec = n.ty().and_then(|t| ast.types.as_record(t)).ok_or(ANON)?;
                let r = *self.recs.get(&rec).ok_or(ANON)?;
                if !anon_class(&self.img, r) || self.img.decls[r as usize].ctx != self.ctx_rec(d)? {
                    return Err(ANON);
                }
                Ok(Some(Payload::AnonObj { rec: r }))
            }
            DeclKind::IndirectField => {
                let chain = ast.indirect_field_chain(d);
                let obj = ast.lists.iter::<DeclId>(chain).next().ok_or(ANON)?;
                let r = *self.recs.get(&obj).ok_or(ANON)?;
                if !matches!(self.img.decls[r as usize].payload, Payload::AnonObj { .. }) {
                    return Err(ANON);
                }
                Ok(Some(Payload::Indirect { obj: r }))
            }
            _ => Ok(None),
        }
    }

    /// Whether field `f` is a member of an anonymous class (at any depth)
    /// of class `class`.
    pub(super) fn anon_member_of(&self, f: DeclId, class: DeclId) -> bool {
        let ast = &self.s.ast;
        let mut c = ast.decls.get(f).ctx();
        for _ in 0..64 {
            match c {
                Some(x) if x == class => return true,
                Some(x)
                    if ast
                        .decls
                        .record(x)
                        .is_some_and(|r| r.defn.contains(DefinitionBits::ANONYMOUS)) =>
                {
                    c = ast.decls.get(x).ctx();
                }
                _ => return false,
            }
        }
        false
    }

    fn ctx_rec(&self, d: DeclId) -> W<u32> {
        let c = self.s.ast.decls.get(d).ctx().ok_or(ANON)?;
        self.recs.get(&c).copied().ok_or(ANON)
    }
}

/// Whether record `r` is a carried anonymous class definition.
pub(super) fn anon_class(img: &Image, r: u32) -> bool {
    img.decls.get(r as usize).is_some_and(|c| {
        c.name == NONE
            && matches!(&c.payload, Payload::Record { def: Some(df), .. } if df.bits & ANON_BIT != 0)
    })
}

/// Whether record `c` is an anonymous class (at any depth) of record
/// `class`.
pub(super) fn anon_within(img: &Image, mut c: u32, class: u32) -> bool {
    for _ in 0..64 {
        if !anon_class(img, c) {
            return false;
        }
        c = img.decls[c as usize].ctx;
        if c == class {
            return true;
        }
    }
    false
}

/// Checks record `i`, an anonymous member object or injected member:
/// it follows what it names, in the same class.
pub(super) fn anon_ok(img: &Image, i: usize, d: &super::DeclRec) -> bool {
    let same_ctx = |x: u32| (x as usize) < i && img.decls[x as usize].ctx == d.ctx;
    match d.payload {
        Payload::AnonObj { rec } => d.name == NONE && anon_class(img, rec) && same_ctx(rec),
        Payload::Indirect { obj } => {
            d.name != NONE
                && (d.name as usize) < img.names.len()
                && same_ctx(obj)
                && matches!(img.decls[obj as usize].payload, Payload::AnonObj { .. })
        }
        _ => false,
    }
}

/// The importer's side.
impl Sema<'_, '_> {
    /// Declares the member object of anonymous class `rec` in class
    /// `h.ctx`, as the parser's declaration without declarators does.
    pub(super) fn image_anon_obj(
        &mut self,
        h: &DeclHead,
        rec: DeclId,
        access: Option<kiln_ast::decl::Access>,
    ) -> DeclId {
        let kw = match self.ast.decls.record(rec).map(|r| r.tag) {
            Some(TagKind::Union) => TagKeyword::Union,
            Some(TagKind::Class) => TagKeyword::Class,
            _ => TagKeyword::Struct,
        };
        let mut ds = DeclSpec::new(h.loc);
        ds.keyword = kw;
        ds.tag = Some(rec);
        ds.tag_declares = true;
        let saved = std::mem::replace(&mut self.ctx, h.ctx);
        let obj = self.act_on_decl_spec_only(&ds);
        self.ctx = saved;
        let Some(obj) = obj else { return self.ast.tu };
        if let Some(a) = access {
            self.ast.decls.set_access(obj, a);
        }
        obj
    }

    /// The member `name` the anonymous member object `obj` injected
    /// into class `ctx`.
    pub(super) fn image_indirect(&self, ctx: DeclId, name: DeclName, obj: DeclId) -> DeclId {
        let ast = &self.ast;
        ast.members(self.class_def(ctx))
            .find(|&m| {
                let n = ast.decls.get(m);
                n.kind() == DeclKind::IndirectField
                    && n.name() == name
                    && ast.lists.iter::<DeclId>(ast.indirect_field_chain(m)).next() == Some(obj)
            })
            .unwrap_or(ast.tu)
    }
}

#[cfg(test)]
mod tests {
    use kiln_ast::types::BuiltinType;

    use super::super::tests::sample;
    use super::super::{DeclRec, QRef, RecDef, decode, encode};
    use super::*;

    /// `sample` plus `struct X { union { int c; }; };`: the class (10),
    /// the anonymous union (11) and its member (12), the member object
    /// (13) and the injected `c` (14).
    fn anon_sample() -> Image {
        let mut img = sample();
        assert_eq!(img.decls.len(), 10);
        let base = img.names.len() as u32;
        img.names.extend(["X", "c"].map(String::from));
        let head = |name, ctx, access, loc, payload| DeclRec {
            flags: 0,
            owner: 3,
            access,
            name,
            ctx,
            lexical: NONE,
            loc,
            prev: NONE,
            payload,
        };
        let def = |tag, bits, end| Payload::Record {
            tag,
            def: Some(RecDef { bits, end, key: NONE, bases: vec![], friends: vec![] }),
        };
        let int = QRef { ty: BuiltinType::Int as u32, quals: 0 };
        img.decls.extend([
            head(base, 1, 0, 200, def(0, 0, 15)),
            head(NONE, 10, 1, 204, def(2, ANON_BIT, 13)),
            head(
                base + 1,
                11,
                1,
                210,
                Payload::Field { ty: int, flags: 0, width: NONE, init: NONE },
            ),
            head(NONE, 10, 1, 204, Payload::AnonObj { rec: 11 }),
            head(base + 1, 10, 1, 210, Payload::Indirect { obj: 13 }),
        ]);
        img
    }

    /// spec/modules.md §11.11 "Forms carried by WP-1345"
    /// ([class.union.anon]): the records round-trip; an object of a class
    /// that is not anonymous, an injected member naming something else
    /// than an object, or with no name, and an anonymous bit on a named
    /// class are errors.
    #[test]
    fn anonymous_member_records() {
        type Corrupt = (&'static str, fn(&mut Image));
        let img = anon_sample();
        assert_eq!(decode(&encode(&img)).as_ref(), Ok(&img));
        let corrupt: [Corrupt; 5] = [
            ("not anonymous", |i| {
                if let Payload::Record { def: Some(df), .. } = &mut i.decls[11].payload {
                    df.bits = 0;
                }
            }),
            ("named anonymous class", |i| i.decls[11].name = i.decls[12].name),
            ("indirect naming a field", |i| i.decls[14].payload = Payload::Indirect { obj: 12 }),
            ("unnamed indirect", |i| i.decls[14].name = NONE),
            ("object in another class", |i| i.decls[13].payload = Payload::AnonObj { rec: 10 }),
        ];
        for (what, f) in corrupt {
            let mut bad = anon_sample();
            f(&mut bad);
            assert!(decode(&encode(&bad)).is_err(), "{what}");
        }
    }
}
