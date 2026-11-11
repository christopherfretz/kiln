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

use kiln_ast::decl::{DeclFlags, DeclKind, FriendKind, FriendTarget, TemplLinkKind};

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
    /// set): a function or a class, in a class that is not a template.
    pub(super) fn friend_gate(&self, d: DeclId) -> W<()> {
        let ast = &self.s.ast;
        let n = ast.decls.get(d);
        match n.kind() {
            DeclKind::Function | DeclKind::CXXRecord => {}
            DeclKind::FunctionTemplate | DeclKind::ClassTemplate => {
                return Err("a friend template");
            }
            _ => return Err("a friend member function"),
        }
        if matches!(ast.templ_link(d).get(), Some((TemplLinkKind::Pattern, _))) {
            return Err("a friend template");
        }
        if n.ctx().is_some_and(|c| ast.decls.get(c).kind() == DeclKind::CXXRecord) {
            return Err("a friend member function");
        }
        if n.has(DeclFlags::DEPENDENT_CTX) {
            return Err("a friend declaration in a class template");
        }
        Ok(())
    }

    /// The payload of friend declaration `d`.
    pub(super) fn friend_record(&mut self, d: DeclId) -> W<Payload> {
        let ast = &self.s.ast;
        if ast.decls.get(d).has(DeclFlags::DEPENDENT_CTX) {
            return Err("a friend declaration in a class template");
        }
        Ok(match ast.decls.friend_target(d) {
            FriendTarget::Decl(t) => {
                if !matches!(ast.decls.get(t).kind(), DeclKind::Function) {
                    return Err("a friend template");
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
    let in_class = class.is_some_and(|c| {
        c.flags & DEP == 0 && matches!(c.payload, Payload::Record { def: Some(_), .. })
    });
    let declared_here = |x: u32, fn_ok: bool| {
        (x as usize) < i
            && img.decls.get(x as usize).is_some_and(|r| {
                kind_bits(r) != 0
                    && r.lexical == d.ctx
                    && if fn_ok {
                        matches!(r.payload, Payload::Function { .. })
                    } else {
                        matches!(r.payload, Payload::Record { def: None, .. })
                    }
            })
    };
    let ok = in_class
        && d.name == NONE
        && d.prev == NONE
        && d.lexical == NONE
        && d.flags & DEP == 0
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

/// The checks of a befriended record `d` (friend bits set): a function
/// or an undefined class, declared in an open class `top` that is not a
/// template, in a namespace.
pub(super) fn friend_context_ok(d: &DeclRec, top: Option<u32>, scope: bool) -> bool {
    kind_bits(d) <= 2
        && d.flags & DEP == 0
        && scope
        && top.is_some_and(|t| d.lexical == t)
        && matches!(d.payload, Payload::Function { .. } | Payload::Record { def: None, .. })
}

/// The checks of class record `c`'s friend list: befriended records
/// before the class's end.
pub(super) fn validate_friend_list(img: &Image, end: u32, friends: &[u32]) -> Res<()> {
    let ok = friends.iter().all(|&f| {
        f < end
            && img.decls.get(f as usize).is_some_and(|r| {
                matches!(r.payload, Payload::Function { .. } | Payload::Record { .. })
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
