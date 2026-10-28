//! Template argument lists (spec/types.md §8.3).
//!
//! A [`TemplateArg`] is 12 bytes: a kind and two operand words. Lists are
//! interned by content and carry their canonical list (computed eagerly)
//! and dependence flags, so argument-list identity is one `u32` compare.

use super::context::TypeContext;
use super::intern::{InternTable, fx_hash};
use super::kinds::TypeFlags;
use crate::ids::{ArgListId, ConstValueId, ExprId, ProfileId, QualType, RawId, TemplateNameId};
use crate::mem::{MemReport, MemRow};

impl ArgListId {
    /// The empty argument list (index 1; created with every context).
    pub const EMPTY: ArgListId = ArgListId::new(1);
}

/// The kind of a template argument.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, PartialOrd, Ord)]
#[repr(u8)]
pub enum ArgKind {
    /// No argument (placeholder in partially deduced lists).
    Null = 0,
    /// A type: `a` = `QualType` raw.
    Type,
    /// A constant value: `a` = its type (`QualType` raw), `b` =
    /// `ConstValueId`.
    Value,
    /// A template: `a` = `TemplateNameId`.
    Template,
    /// A template pack expansion `TT...`: `a` = `TemplateNameId`, `b` =
    /// number of expansions + 1 (0 = unknown).
    TemplateExpansion,
    /// An expression as written: `a` = `ExprId`, `b` = its profile when
    /// dependent (0 otherwise).
    Expr,
    /// The canonical form of a dependent expression argument: `a` =
    /// `ProfileId`.
    Profile,
    /// An argument pack: `a` = `ArgListId`.
    Pack,
}

/// One template argument: 12 bytes.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct TemplateArg {
    pub(crate) kind: ArgKind,
    pub(crate) a: u32,
    pub(crate) b: u32,
}

/// A decoded template argument.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[allow(missing_docs)]
pub enum ArgView {
    Null,
    Type(QualType),
    Value { ty: QualType, value: ConstValueId },
    Template(TemplateNameId),
    TemplateExpansion { name: TemplateNameId, num_expansions: Option<u32> },
    Expr { expr: ExprId, profile: Option<ProfileId> },
    Profile(ProfileId),
    Pack(ArgListId),
}

impl TemplateArg {
    /// The null argument.
    pub const NULL: TemplateArg = TemplateArg { kind: ArgKind::Null, a: 0, b: 0 };

    /// A type argument.
    pub const fn ty(q: QualType) -> TemplateArg {
        TemplateArg { kind: ArgKind::Type, a: q.to_raw(), b: 0 }
    }

    /// A constant-value argument of type `ty`.
    pub fn value(ty: QualType, value: ConstValueId) -> TemplateArg {
        TemplateArg { kind: ArgKind::Value, a: ty.to_raw(), b: value.raw() }
    }

    /// A template argument.
    pub fn template(n: TemplateNameId) -> TemplateArg {
        TemplateArg { kind: ArgKind::Template, a: n.raw(), b: 0 }
    }

    /// A template pack expansion `TT...`.
    pub fn template_expansion(n: TemplateNameId, num_expansions: Option<u32>) -> TemplateArg {
        let b = num_expansions.and_then(|n| n.checked_add(1)).unwrap_or(0);
        TemplateArg { kind: ArgKind::TemplateExpansion, a: n.raw(), b }
    }

    /// An expression argument; `profile` is required when the expression
    /// is value- or instantiation-dependent (spec/types.md §7.2).
    pub fn expr(e: ExprId, profile: Option<ProfileId>) -> TemplateArg {
        TemplateArg { kind: ArgKind::Expr, a: e.raw(), b: ProfileId::raw_opt(profile) }
    }

    /// The canonical form of a dependent expression argument.
    pub fn profile(p: ProfileId) -> TemplateArg {
        TemplateArg { kind: ArgKind::Profile, a: p.raw(), b: 0 }
    }

    /// An argument pack.
    pub fn pack(list: ArgListId) -> TemplateArg {
        TemplateArg { kind: ArgKind::Pack, a: list.raw(), b: 0 }
    }

    /// The argument's kind.
    pub const fn kind(&self) -> ArgKind {
        self.kind
    }

    /// The argument, decoded.
    pub fn view(&self) -> ArgView {
        let q = |v: u32| QualType::from_raw(v).unwrap_or(QualType::ERROR);
        let n = |v: u32| TemplateNameId::try_new(v).unwrap_or(TemplateNameId::new(1));
        match self.kind {
            ArgKind::Null => ArgView::Null,
            ArgKind::Type => ArgView::Type(q(self.a)),
            ArgKind::Value => match ConstValueId::try_new(self.b) {
                Some(value) => ArgView::Value { ty: q(self.a), value },
                None => ArgView::Null,
            },
            ArgKind::Template => ArgView::Template(n(self.a)),
            ArgKind::TemplateExpansion => ArgView::TemplateExpansion {
                name: n(self.a),
                num_expansions: self.b.checked_sub(1),
            },
            ArgKind::Expr => ArgView::Expr {
                expr: ExprId::try_new(self.a).unwrap_or(ExprId::new(1)),
                profile: ProfileId::try_new(self.b),
            },
            ArgKind::Profile => {
                ArgView::Profile(ProfileId::try_new(self.a).unwrap_or(ProfileId::new(1)))
            }
            ArgKind::Pack => ArgView::Pack(ArgListId::try_new(self.a).unwrap_or(ArgListId::EMPTY)),
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct ListEntry {
    start: u32,
    len: u32,
    canon: u32,
    flags: TypeFlags,
}

/// The argument arena and the list intern table.
#[derive(Debug)]
pub(crate) struct ArgTable {
    args: Vec<TemplateArg>,
    lists: Vec<ListEntry>,
    intern: InternTable,
}

impl ArgTable {
    pub(crate) fn new() -> ArgTable {
        ArgTable {
            args: Vec::new(),
            lists: vec![ListEntry { start: 0, len: 0, canon: 0, flags: TypeFlags::NONE }],
            intern: InternTable::default(),
        }
    }

    fn slice(&self, e: &ListEntry) -> &[TemplateArg] {
        &self.args[e.start as usize..e.start as usize + e.len as usize]
    }

    fn find(&self, args: &[TemplateArg]) -> Result<u32, u64> {
        let hash = fx_hash(args);
        self.intern.find(hash, |id| self.slice(&self.lists[id as usize]) == args).ok_or(hash)
    }

    fn push(
        &mut self,
        args: &[TemplateArg],
        hash: u64,
        canon: Option<u32>,
        flags: TypeFlags,
    ) -> Option<u32> {
        let id = u32::try_from(self.lists.len()).ok().filter(|&i| i != u32::MAX)?;
        let end = self.args.len().checked_add(args.len())?;
        if end > u32::MAX as usize {
            return None;
        }
        let start = self.args.len() as u32;
        self.args.extend_from_slice(args);
        self.lists.push(ListEntry {
            start,
            len: args.len() as u32,
            canon: canon.unwrap_or(id),
            flags,
        });
        let (lists, all) = (&self.lists, &self.args);
        self.intern.insert(hash, id, |i| {
            let e = &lists[i as usize];
            fx_hash(&all[e.start as usize..e.start as usize + e.len as usize])
        });
        Some(id)
    }

    pub(crate) fn memory(&self, r: &mut MemReport) {
        r.push(MemRow::of_vec("args.args", &self.args));
        r.push(MemRow::of_vec("args.lists", &self.lists));
        r.push(MemRow::new(
            "args.intern",
            self.intern.len() as u64,
            self.intern.live_bytes(),
            self.intern.heap_bytes(),
        ));
    }
}

impl TypeContext {
    /// Interns an argument list. Returns [`ArgListId::EMPTY`] and sets
    /// [`TypeContext::exhausted`] when the arena is full.
    pub fn arg_list(&mut self, args: &[TemplateArg]) -> ArgListId {
        let hash = match self.args.find(args) {
            Ok(id) => return ArgListId::new(id),
            Err(h) => h,
        };
        let mut flags = TypeFlags::NONE;
        let mut canon: Vec<TemplateArg> = Vec::with_capacity(args.len());
        for a in args {
            flags |= self.arg_flags(*a);
            canon.push(self.canonical_arg(*a));
        }
        let c = if canon.as_slice() == args { None } else { Some(self.arg_list(&canon).index()) };
        if let Some(id) = self.args.push(args, hash, c, flags.without(TypeFlags::SUGAR)) {
            ArgListId::new(id)
        } else {
            self.exhausted = true;
            ArgListId::EMPTY
        }
    }

    /// The arguments of list `l`.
    pub fn args(&self, l: ArgListId) -> &[TemplateArg] {
        self.args.slice(&self.args.lists[l.index() as usize])
    }

    /// The canonical form of list `l`.
    pub fn canonical_args(&self, l: ArgListId) -> ArgListId {
        ArgListId::new(self.args.lists[l.index() as usize].canon)
    }

    /// The union of the dependence flags of the arguments of `l`.
    pub fn arg_list_flags(&self, l: ArgListId) -> TypeFlags {
        self.args.lists[l.index() as usize].flags
    }

    /// The canonical form of one argument.
    pub fn canonical_arg(&self, a: TemplateArg) -> TemplateArg {
        match a.view() {
            ArgView::Type(q) => TemplateArg::ty(self.canonical(q)),
            ArgView::Value { ty, value } => TemplateArg::value(self.canonical(ty), value),
            ArgView::Template(n) => TemplateArg::template(self.canonical_template_name(n)),
            ArgView::TemplateExpansion { name, num_expansions } => {
                TemplateArg::template_expansion(self.canonical_template_name(name), num_expansions)
            }
            ArgView::Expr { profile: Some(p), .. } => TemplateArg::profile(p),
            ArgView::Null | ArgView::Profile(_) | ArgView::Expr { profile: None, .. } => a,
            ArgView::Pack(l) => TemplateArg::pack(self.canonical_args(l)),
        }
    }

    /// The dependence flags of one argument.
    pub fn arg_flags(&self, a: TemplateArg) -> TypeFlags {
        match a.view() {
            ArgView::Null => TypeFlags::NONE,
            ArgView::Type(q) => self.qflags(q),
            ArgView::Value { ty, .. } => self.qflags(ty),
            ArgView::Template(n) => self.template_name_flags(n),
            ArgView::TemplateExpansion { name, .. } => {
                self.template_name_flags(name).without(TypeFlags::UNEXPANDED_PACK)
                    | TypeFlags::DEPENDENT_INSTANTIATION
            }
            ArgView::Expr { profile, .. } => {
                profile.map_or(TypeFlags::NONE, |p| self.profile_flags(p))
            }
            ArgView::Profile(p) => self.profile_flags(p),
            ArgView::Pack(l) => self.arg_list_flags(l),
        }
    }
}
