//! Class members of WP-1345 in the declaration image (spec/modules.md
//! §11.11, "Forms carried by WP-1345").
//!
//! An implicitly declared special member of a class template
//! specialization the unit instantiated ([class.default.ctor]/1,
//! [class.copy.ctor]/6, [class.dtor]/2): a body that names it (an
//! `inline holder<str> h;`, a temporary's destructor) names an
//! [`Payload::Implicit`] record whose context is the specialization
//! reference (a complete [`Payload::Spec`]); the importer, which
//! instantiated the class at that record, declares the member again as
//! sema does lazily, and defines it again when the unit used it.

use kiln_ast::DeclId;
use kiln_ast::decl::{DeclFlags, DeclKind, Defaulted};
use kiln_ast::templates::SpecKind;

use super::{DECL_FLAGS, DeclRec, Image, NONE, Payload, W, Writer};

impl Writer<'_, '_, '_> {
    /// Records the implicit special members the unit used or referenced
    /// of each complete specialization reference (an inline variable's
    /// destructor, which no body names), once the declarations are in.
    pub(super) fn spec_implicit_members(&mut self) -> W<()> {
        let specs = std::mem::take(&mut self.complete_specs);
        for (d, rec) in specs {
            // A reference rolled back with the declaration that made it.
            if self.recs.get(&d) != Some(&rec) {
                continue;
            }
            let s = self.s;
            let ast = &s.ast;
            // Every one sema declared, in member order (a destructor no
            // body names, an inline variable's, is used too).
            let ms: Vec<DeclId> = ast
                .members(d)
                .filter(|&m| {
                    ast.decls.get(m).has(DeclFlags::IMPLICIT) && s.special_kind_of(m).is_some()
                })
                .collect();
            for m in ms {
                if let Err(why) = self.implicit_spec_member(m, d) {
                    self.miss(rec, why)?;
                }
            }
        }
        Ok(())
    }

    /// The order the writer visits the declarations in: creation order,
    /// except that an explicit specialization of a class template a
    /// declaration named before the specialization was declared
    /// (`template <class T> inline const bool v<plus<void>, T> = true;`
    /// before `template <> struct plus<void> { ... };`, which sema makes
    /// at the first name and specializes in place) is visited just before
    /// its first member, where replay declares it. The declarations in
    /// between name it through a specialization reference
    /// ([`Writer::spec_rec`]), as their replay named an undeclared
    /// specialization.
    pub(super) fn collect_order(&mut self) -> Vec<DeclId> {
        let ast = &self.s.ast;
        let mut order: Vec<DeclId> = Vec::with_capacity(ast.decls.ids().len());
        let mut at = kiln_source::fx::FxHashMap::<DeclId, Vec<DeclId>>::default();
        for d in ast.decls.ids() {
            if let Some(v) = at.remove(&d) {
                order.extend(v);
            }
            match self.deferred_until(d) {
                Some(m) => {
                    at.entry(m).or_default().push(d);
                    let _ = self.deferred.insert(d);
                }
                None => order.push(d),
            }
        }
        order.extend(at.into_values().flatten());
        order
    }

    /// The first declaration inside `d` when `d` is an explicit class
    /// template specialization definition whose members do not follow it
    /// (another declaration was made between them).
    fn deferred_until(&self, d: DeclId) -> Option<DeclId> {
        let ast = &self.s.ast;
        let n = ast.decls.get(d);
        if n.kind() != DeclKind::CXXRecord {
            return None;
        }
        let (_, si) = ast.spec_info(d)?;
        if si.kind != SpecKind::ExplicitSpecialization || si.from != d {
            return None;
        }
        let def = ast.definition(d)?;
        let first = ast.members(def).min()?;
        if first.index() <= d.index() + 1 {
            return None;
        }
        // Something other than the class's own declarations in between.
        let inside = |x: DeclId| {
            let mut c = ast.decls.get(x).ctx();
            for _ in 0..256 {
                match c {
                    Some(y) if y == d => return true,
                    Some(y) => c = ast.decls.get(y).ctx(),
                    None => return false,
                }
            }
            false
        };
        let m = (d.index() + 1..=first.index()).map(DeclId::new).find(|&x| inside(x))?;
        (m.index() > d.index() + 1).then_some(m)
    }

    /// The record of implicit member `d` of specialization `class`.
    pub(super) fn implicit_spec_member(&mut self, d: DeclId, class: DeclId) -> W<u32> {
        if let Some(&r) = self.recs.get(&d) {
            return Ok(r);
        }
        let s = self.s;
        let ast = &s.ast;
        let n = ast.decls.get(d);
        let k =
            s.special_kind_of(d).ok_or("an implicit declaration that is not a special member")?;
        let implicit_default =
            ast.decls.function(d).is_some_and(|f| f.spec.defaulted() == Defaulted::Implicit);
        if !implicit_default || n.flags().bits() & !DECL_FLAGS != DeclFlags::IMPLICIT.bits() {
            return Err("an implicit special member that is deleted, not defaulted, or flagged");
        }
        let spec = self.spec_rec(class)?.ok_or("a member of a nested class of a specialization")?;
        if !matches!(self.img.decls[spec as usize].payload, Payload::Spec { complete: true, .. }) {
            return Err("an implicit member of a class template specialization");
        }
        // The implicit members sema declared before it first, so the
        // importer declares them in the same order (overload candidates
        // are listed in member order).
        let before: Vec<DeclId> = ast
            .members(class)
            .take_while(|&m| m != d)
            .filter(|&m| {
                ast.decls.get(m).has(DeclFlags::IMPLICIT)
                    && s.special_kind_of(m).is_some()
                    && !self.recs.contains_key(&m)
            })
            .collect();
        for m in before {
            self.implicit_spec_member(m, class)?;
        }
        let s = self.s;
        let ast = &s.ast;
        let o = ast.modules.owner(d);
        let i = self.img.decls.len() as u32;
        self.img.decls.push(DeclRec {
            flags: n.flags().bits() & DECL_FLAGS,
            owner: u8::from(o.purview) | (u8::from(o.exported) << 1),
            access: 0,
            name: NONE,
            ctx: spec,
            lexical: NONE,
            loc: NONE,
            prev: NONE,
            payload: Payload::Implicit { kind: k as u8 },
        });
        self.recs.insert(d, i);
        Ok(i)
    }
}

/// Whether record `x` is a specialization reference the importer
/// completes (the context of an implicit member, WP-1345).
pub(super) fn complete_spec(img: &Image, x: u32) -> bool {
    matches!(
        img.decls.get(x as usize).map(|r| &r.payload),
        Some(Payload::Spec { complete: true, .. })
    )
}
