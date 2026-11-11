//! Friends in the declaration image (spec/modules.md §11.11, "Friends";
//! WP-1292).
//!
//! A friend declaration ([class.friend]) of a class that is not a
//! template is carried as three things:
//!
//! - the befriended function, or the class an elaborated
//!   `friend struct H;` declares, as an ordinary record whose context is
//!   its namespace and whose lexical context is the class, with its
//!   [`FriendKind`] in bits 2–3 of the owner byte. A hidden friend
//!   (`FriendKind::Undeclared`) is entered by the reader's builder as
//!   sema's is, found only by argument-dependent lookup
//!   ([namespace.memdef]/3, [basic.lookup.argdep]/4);
//! - the `Friend` record in the class, after it, naming that record or
//!   the befriended type;
//! - the class's friend list (the first declarations access checks
//!   consult, spec/sema-class.md §2.4), written at `}`, as record
//!   indices in the class's definition.
//!
//! Friends of class templates, friend templates and member functions
//! befriended by a qualified name still fall back.

use kiln_ast::decl::{DeclFlags, DeclKind, FriendKind, FriendTarget};

use crate::Sema;
use kiln_ast::{DeclHead, DeclId};

use super::{DEP, DeclRec, Image, NONE, Payload, Res, W, Writer, bad};

/// The friend kind of record `d` (bits 2–3 of its owner byte).
pub(super) fn kind_bits(d: &DeclRec) -> u8 {
    d.owner >> 2
}

/// The owner-byte bits of friend kind `k`.
pub(super) fn owner_bits(k: FriendKind) -> u8 {
    (k as u8) << 2
}

/// The [`FriendKind`] of owner bits `b` (validated).
pub(super) fn kind_of_bits(b: u8) -> FriendKind {
    match b {
        1 => FriendKind::Declared,
        2 => FriendKind::Undeclared,
        _ => FriendKind::None,
    }
}

impl Writer<'_, '_, '_> {
    /// A declaration a friend declaration declared (`misc.friend()`
    /// set): a function or a class, a friend function or class template
    /// (its pattern and its template record; WP-1307), in a class that
    /// is not a template.
    pub(super) fn friend_gate(&self, d: DeclId) -> W<()> {
        let ast = &self.s.ast;
        let n = ast.decls.get(d);
        if !matches!(
            n.kind(),
            DeclKind::Function
                | DeclKind::CXXRecord
                | DeclKind::FunctionTemplate
                | DeclKind::ClassTemplate
        ) {
            return Err("a friend member function");
        }
        if n.ctx().is_some_and(|c| ast.decls.get(c).kind() == DeclKind::CXXRecord) {
            return Err("a friend member function");
        }
        if self.in_class_template(d) && self.dep_friend_class(d).is_none() {
            return Err("a friend declaration in a class template");
        }
        let s = self.s;
        if s.friend_qualifiers.contains_key(&d) || s.friend_qualifier_nns.contains_key(&d) {
            return Err("a friend declared by a qualified name");
        }
        Ok(())
    }

    /// Whether befriended declaration `d` was declared in a class
    /// template (its lexical context, a dependent class).
    fn in_class_template(&self, d: DeclId) -> bool {
        let ast = &self.s.ast;
        let lex = ast.decls.lexical_ctx(d).unwrap_or(ast.tu);
        ast.decls.get(lex).has(DeclFlags::DEPENDENT_CTX)
    }

    /// The record of the carried class template pattern (a dependent
    /// class) befriended declaration `d` was declared in, its lexical
    /// context (WP-1307): `d` is then a member of that pattern as its
    /// members are, instantiated with it ([temp.friend]).
    pub(super) fn dep_friend_class(&self, d: DeclId) -> Option<u32> {
        let ast = &self.s.ast;
        let n = ast.decls.get(d);
        if n.misc().friend() == FriendKind::None || !n.has(DeclFlags::HAS_LEXICAL_CTX) {
            return None;
        }
        let lex = ast.decls.lexical_ctx(d)?;
        let r = *self.recs.get(&lex)?;
        let c = &self.img.decls[r as usize];
        (c.flags & DEP != 0 && matches!(c.payload, Payload::Record { .. })).then_some(r)
    }

    /// Whether `d` is the `FriendDecl` of a friend template: sema makes
    /// it inside the template's head, so it is dependent even in a class
    /// that is not a template (WP-1307).
    pub(super) fn template_friend_decl(&self, d: DeclId) -> bool {
        let ast = &self.s.ast;
        ast.decls.get(d).kind() == DeclKind::Friend
            && matches!(ast.decls.friend_target(d), FriendTarget::Decl(t)
                if matches!(ast.decls.get(t).kind(), DeclKind::FunctionTemplate | DeclKind::ClassTemplate))
    }

    /// The payload of friend declaration `d`.
    pub(super) fn friend_record(&mut self, d: DeclId) -> W<Payload> {
        let ast = &self.s.ast;
        let class = ast.decls.get(d).ctx().unwrap_or(ast.tu);
        if ast.decls.get(d).has(DeclFlags::DEPENDENT_CTX)
            && !self.template_friend_decl(d)
            && !ast.decls.get(class).has(DeclFlags::DEPENDENT_CTX)
        {
            return Err("a friend declaration in a template");
        }
        if self.s.friend_specs.contains_key(&d) || self.s.member_friends.by_friend.contains_key(&d)
        {
            return Err("a friend specialization or member of a class template");
        }
        Ok(match ast.decls.friend_target(d) {
            FriendTarget::Decl(t) => {
                if !matches!(
                    ast.decls.get(t).kind(),
                    DeclKind::Function | DeclKind::FunctionTemplate | DeclKind::ClassTemplate
                ) {
                    return Err("a friend member function");
                }
                let target = self.recs.get(&t).copied().ok_or("a friend outside the image")?;
                Payload::Friend { target, ty: None, owned: NONE }
            }
            FriendTarget::Type { ty, owned_tag } => {
                let owned = match owned_tag {
                    None => NONE,
                    Some(o) => self.recs.get(&o).copied().ok_or("a friend outside the image")?,
                };
                Payload::Friend { target: NONE, ty: Some(self.qref(ty)?), owned }
            }
        })
    }

    /// The friend lists of the carried classes (after the walk: a
    /// hidden friend follows its class's record).
    pub(super) fn friend_lists(&mut self) -> W<()> {
        let ast = &self.s.ast;
        let mut lists: Vec<(u32, Vec<DeclId>)> = self
            .recs
            .iter()
            .filter_map(|(&d, &i)| {
                let l = ast.decls.cold.friends.get(d)?;
                Some((i, ast.lists.iter::<DeclId>(*l).collect()))
            })
            .collect();
        lists.sort_unstable_by_key(|(i, _)| *i);
        for (i, list) in lists {
            if !matches!(self.img.decls[i as usize].payload, Payload::Record { def: Some(_), .. }) {
                continue;
            }
            let mut out = Vec::with_capacity(list.len());
            for f in list {
                match self.recs.get(&f) {
                    Some(&r) => out.push(r),
                    None => self.miss(i, "a friend outside the image")?,
                }
            }
            if let Payload::Record { def: Some(df), .. } = &mut self.img.decls[i as usize].payload {
                df.friends = out;
            }
        }
        Ok(())
    }
}

/// The checks of a friend declaration record `i` (`d`): in a class that
/// is not a template, naming a befriended record declared by it.
pub(super) fn validate_friend(img: &Image, i: usize, d: &DeclRec) -> Res<()> {
    let Payload::Friend { target, ty, owned } = d.payload else { return Ok(()) };
    let class = img.decls.get(d.ctx as usize);
    // In a class, or in a class template's pattern (WP-1307), whose
    // `Friend` records are dependent.
    let dep_class = class.is_some_and(|c| c.flags & DEP != 0);
    let in_class = class.is_some_and(|c| matches!(c.payload, Payload::Record { def: Some(_), .. }));
    let declared_here = |x: u32, fn_ok: bool| {
        (x as usize) < i
            && img.decls.get(x as usize).is_some_and(|r| {
                kind_bits(r) != 0
                    && r.lexical == d.ctx
                    && if fn_ok {
                        matches!(
                            r.payload,
                            Payload::Function { .. } | Payload::Template { kind: 0 | 1, .. }
                        )
                    } else {
                        matches!(r.payload, Payload::Record { def: None, .. })
                    }
            })
    };
    let ok = in_class
        && d.name == NONE
        && d.prev == NONE
        && d.lexical == NONE
        && (d.flags & DEP != 0) == (dep_class || names_template(img, d))
        && match (target, ty) {
            (t, None) => declared_here(t, true) && owned == NONE,
            (NONE, Some(_)) => owned == NONE || declared_here(owned, false),
            _ => false,
        };
    if !ok {
        return bad("bad friend declaration");
    }
    Ok(())
}

/// Whether `Friend` record `d` names a friend template (it is then
/// dependent: sema declares it inside the template's head).
pub(super) fn names_template(img: &Image, d: &DeclRec) -> bool {
    matches!(d.payload, Payload::Friend { target, ty: None, .. }
        if img.decls.get(target as usize).is_some_and(|r| matches!(r.payload, Payload::Template { .. })))
}

/// The checks of a befriended record `d` (friend bits set): a function
/// or an undefined class, or a friend template's pattern or template
/// record (WP-1307; dependent, claimed as any pattern), declared in an
/// open class `top`, in a namespace.
pub(super) fn friend_context_ok(d: &DeclRec, top: Option<u32>, scope: bool) -> bool {
    kind_bits(d) <= 2
        && scope
        && top.is_some_and(|t| d.lexical == t)
        && match d.payload {
            Payload::Function { .. } | Payload::Record { def: None, .. } => true,
            Payload::Template { kind, .. } => kind <= 1,
            _ => false,
        }
}

/// The context whose template-heads enclose a template-head declared by
/// record `d` in context `ctx`: a friend template's is the class that
/// declared it, its lexical context ([temp.friend]/1; WP-1307).
pub(super) fn head_ctx(d: &DeclRec) -> u32 {
    if kind_bits(d) != 0 && d.lexical != NONE && d.lexical != super::LEX_TU {
        d.lexical
    } else {
        d.ctx
    }
}

/// The checks of class record `c`'s friend list: befriended records
/// before the class's end.
pub(super) fn validate_friend_list(img: &Image, end: u32, friends: &[u32]) -> Res<()> {
    let ok = friends.iter().all(|&f| {
        f < end
            && img.decls.get(f as usize).is_some_and(|r| {
                matches!(
                    r.payload,
                    Payload::Function { .. }
                        | Payload::Record { .. }
                        | Payload::Template { kind: 0 | 1, .. }
                )
            })
    });
    if !ok || friends.windows(2).any(|w| w[0] == w[1]) {
        return bad("bad friend list");
    }
    Ok(())
}

impl Sema<'_, '_> {
    /// The friend list of class `c` (image record `ri`), before its `}`.
    pub(super) fn image_friends(&mut self, img: &Image, ids: &[DeclId], c: DeclId, ri: usize) {
        let Some(Payload::Record { def: Some(df), .. }) = img.decls.get(ri).map(|r| &r.payload)
        else {
            return;
        };
        let list: Vec<DeclId> =
            df.friends.iter().filter_map(|&f| ids.get(f as usize).copied()).collect();
        let _ = self.builder().set_friends(c, &list);
    }
}

/// The reader's half of a befriended record: its head's friend kind.
pub(super) fn friend_head(h: &mut DeclHead, r: &DeclRec) {
    let k = kind_of_bits(kind_bits(r));
    if k != FriendKind::None {
        h.misc = h.misc.with_friend(k);
    }
}
