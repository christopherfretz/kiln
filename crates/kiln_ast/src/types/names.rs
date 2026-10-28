//! Template names (spec/types.md §8.1).

use super::context::TypeContext;
use super::kinds::TypeFlags;
use crate::ids::{ArgListId, DeclId, ExprId, NnsId, ProfileId, RawId, TemplateNameId};
use kiln_source::symbol::Symbol;

/// A set of function templates named by an unresolved template-id
/// (`f<int>` before overload resolution). Stored in the name table's
/// declaration pool; see [`TypeContext::name_decls`].
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct DeclSet {
    pub(crate) start: u32,
    pub(crate) len: u32,
}

impl DeclSet {
    /// The empty set.
    pub const EMPTY: DeclSet = DeclSet { start: 0, len: 0 };

    /// Number of declarations.
    pub const fn len(self) -> u32 {
        self.len
    }

    /// True for the empty set.
    pub const fn is_empty(self) -> bool {
        self.len == 0
    }
}

/// What a template name refers to.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum TemplateName {
    /// A class, alias, variable, function or concept template
    /// (the first declaration).
    Template(DeclId),
    /// `nns::[template] X`: sugar over `underlying`.
    Qualified {
        /// The qualifier as written.
        nns: NnsId,
        /// The `template` keyword was written.
        template_kw: bool,
        /// The name without the qualifier.
        underlying: TemplateNameId,
    },
    /// `nns::template name` with a dependent `nns`.
    DependentIdentifier {
        /// The dependent qualifier.
        nns: NnsId,
        /// The identifier.
        name: Symbol,
    },
    /// `nns::template operator@` with a dependent `nns`.
    DependentOperator {
        /// The dependent qualifier.
        nns: NnsId,
        /// The overloaded operator (`kiln_lex` `TokenKind` as `u8`).
        op: u8,
    },
    /// A template template parameter as written.
    TemplateTemplateParm {
        /// The parameter declaration.
        decl: DeclId,
        /// Template depth.
        depth: u32,
        /// Index in its parameter list.
        index: u32,
        /// It is a parameter pack.
        pack: bool,
    },
    /// The canonical template template parameter at `(depth, index)`.
    CanonTemplateTemplateParm {
        /// Template depth.
        depth: u32,
        /// Index in its parameter list.
        index: u32,
        /// It is a parameter pack.
        pack: bool,
    },
    /// A substituted template template parameter: sugar over
    /// `replacement`.
    SubstTemplateTemplateParm {
        /// The template argument.
        replacement: TemplateNameId,
        /// The declaration whose parameter was substituted.
        associated: DeclId,
        /// The parameter's index.
        index: u32,
    },
    /// A template template parameter pack being substituted.
    SubstTemplateTemplateParmPack {
        /// The declaration whose parameter is substituted.
        associated: DeclId,
        /// The parameter's index.
        index: u32,
        /// The argument pack.
        args: ArgListId,
    },
    /// Named through a using-declaration: sugar over `underlying`.
    Using {
        /// The using-shadow declaration.
        shadow: DeclId,
        /// The target name.
        underlying: TemplateNameId,
    },
    /// An overload set of function templates.
    Overloaded(DeclSet),
    /// `[: expr :]` naming a template (P2996, M6).
    Splice {
        /// The splice expression.
        expr: ExprId,
        /// Its profile when dependent (the canonical form is then
        /// [`TemplateName::SpliceProfile`]).
        profile: Option<ProfileId>,
    },
    /// The canonical form of a dependent splice.
    SpliceProfile(ProfileId),
}

const K_TEMPLATE: u32 = 1;
const K_QUALIFIED: u32 = 2;
const K_DEP_ID: u32 = 3;
const K_DEP_OP: u32 = 4;
const K_TTP: u32 = 5;
const K_CANON_TTP: u32 = 6;
const K_SUBST: u32 = 7;
const K_SUBST_PACK: u32 = 8;
const K_USING: u32 = 9;
const K_OVERLOADED: u32 = 10;
const K_SPLICE: u32 = 11;
const K_SPLICE_PROFILE: u32 = 12;

fn encode(n: TemplateName) -> [u32; 4] {
    match n {
        TemplateName::Template(d) => [K_TEMPLATE, d.raw(), 0, 0],
        TemplateName::Qualified { nns, template_kw, underlying } => {
            [K_QUALIFIED | (u32::from(template_kw) << 8), nns.raw(), underlying.raw(), 0]
        }
        TemplateName::DependentIdentifier { nns, name } => [K_DEP_ID, nns.raw(), name.index(), 0],
        TemplateName::DependentOperator { nns, op } => [K_DEP_OP, nns.raw(), u32::from(op), 0],
        TemplateName::TemplateTemplateParm { decl, depth, index, pack } => {
            [K_TTP | (u32::from(pack) << 8), decl.raw(), depth, index]
        }
        TemplateName::CanonTemplateTemplateParm { depth, index, pack } => {
            [K_CANON_TTP | (u32::from(pack) << 8), 0, depth, index]
        }
        TemplateName::SubstTemplateTemplateParm { replacement, associated, index } => {
            [K_SUBST, replacement.raw(), associated.raw(), index]
        }
        TemplateName::SubstTemplateTemplateParmPack { associated, index, args } => {
            [K_SUBST_PACK, associated.raw(), index, args.raw()]
        }
        TemplateName::Using { shadow, underlying } => [K_USING, shadow.raw(), underlying.raw(), 0],
        TemplateName::Overloaded(s) => [K_OVERLOADED, s.start, s.len, 0],
        TemplateName::Splice { expr, profile } => {
            [K_SPLICE, expr.raw(), ProfileId::raw_opt(profile), 0]
        }
        TemplateName::SpliceProfile(p) => [K_SPLICE_PROFILE, p.raw(), 0, 0],
    }
}

fn decode(k: [u32; 4]) -> TemplateName {
    let d = |v: u32| DeclId::try_new(v).unwrap_or(DeclId::new(1));
    let n = |v: u32| TemplateNameId::try_new(v).unwrap_or(TemplateNameId::new(1));
    let nns = |v: u32| NnsId::try_new(v).unwrap_or(NnsId::new(1));
    let flag = k[0] >> 8 != 0;
    match k[0] & 0xff {
        K_TEMPLATE => TemplateName::Template(d(k[1])),
        K_QUALIFIED => {
            TemplateName::Qualified { nns: nns(k[1]), template_kw: flag, underlying: n(k[2]) }
        }
        K_DEP_ID => {
            TemplateName::DependentIdentifier { nns: nns(k[1]), name: Symbol::from_index(k[2]) }
        }
        K_DEP_OP => TemplateName::DependentOperator { nns: nns(k[1]), op: k[2] as u8 },
        K_TTP => TemplateName::TemplateTemplateParm {
            decl: d(k[1]),
            depth: k[2],
            index: k[3],
            pack: flag,
        },
        K_CANON_TTP => {
            TemplateName::CanonTemplateTemplateParm { depth: k[2], index: k[3], pack: flag }
        }
        K_SUBST => TemplateName::SubstTemplateTemplateParm {
            replacement: n(k[1]),
            associated: d(k[2]),
            index: k[3],
        },
        K_SUBST_PACK => TemplateName::SubstTemplateTemplateParmPack {
            associated: d(k[1]),
            index: k[2],
            args: ArgListId::try_new(k[3]).unwrap_or(ArgListId::EMPTY),
        },
        K_USING => TemplateName::Using { shadow: d(k[1]), underlying: n(k[2]) },
        K_SPLICE => TemplateName::Splice {
            expr: ExprId::try_new(k[1]).unwrap_or(ExprId::new(1)),
            profile: ProfileId::try_new(k[2]),
        },
        K_SPLICE_PROFILE => {
            TemplateName::SpliceProfile(ProfileId::try_new(k[1]).unwrap_or(ProfileId::new(1)))
        }
        _ => TemplateName::Overloaded(DeclSet { start: k[1], len: k[2] }),
    }
}

impl TypeContext {
    /// Interns a template name. Returns the empty-overload-set sentinel
    /// (id 1) and sets [`TypeContext::exhausted`] when the table is full.
    pub fn template_name(&mut self, n: TemplateName) -> TemplateNameId {
        let key = encode(n);
        let hash = match self.names.find(&key) {
            Ok(id) => return TemplateNameId::new(id),
            Err(h) => h,
        };
        let dep = TypeFlags::DEPENDENT_INSTANTIATION;
        let pack = |p: bool| if p { TypeFlags::UNEXPANDED_PACK } else { TypeFlags::NONE };
        let (canon, flags): (Option<TemplateNameId>, TypeFlags) = match n {
            TemplateName::Template(_) | TemplateName::Overloaded(_) => (None, TypeFlags::NONE),
            TemplateName::Qualified { nns, underlying, .. } => (
                Some(self.canonical_template_name(underlying)),
                self.template_name_flags(underlying) | self.nns_flags(nns),
            ),
            TemplateName::DependentIdentifier { nns, name } => {
                let c = self.canonical_nns(nns);
                let canon = (c != nns).then(|| {
                    self.template_name(TemplateName::DependentIdentifier { nns: c, name })
                });
                (canon, dep | self.nns_flags(nns))
            }
            TemplateName::DependentOperator { nns, op } => {
                let c = self.canonical_nns(nns);
                let canon = (c != nns)
                    .then(|| self.template_name(TemplateName::DependentOperator { nns: c, op }));
                (canon, dep | self.nns_flags(nns))
            }
            TemplateName::TemplateTemplateParm { depth, index, pack: p, .. } => (
                Some(self.template_name(TemplateName::CanonTemplateTemplateParm {
                    depth,
                    index,
                    pack: p,
                })),
                dep | pack(p),
            ),
            TemplateName::CanonTemplateTemplateParm { pack: p, .. } => (None, dep | pack(p)),
            TemplateName::SubstTemplateTemplateParm { replacement, .. } => (
                Some(self.canonical_template_name(replacement)),
                self.template_name_flags(replacement),
            ),
            TemplateName::SubstTemplateTemplateParmPack { associated, index, args } => {
                let c = self.canonical_args(args);
                let canon = (c != args).then(|| {
                    self.template_name(TemplateName::SubstTemplateTemplateParmPack {
                        associated,
                        index,
                        args: c,
                    })
                });
                (canon, dep | TypeFlags::UNEXPANDED_PACK)
            }
            TemplateName::Using { underlying, .. } => (
                Some(self.canonical_template_name(underlying)),
                self.template_name_flags(underlying),
            ),
            TemplateName::Splice { profile, .. } => {
                let pf = profile.map_or(TypeFlags::NONE, |p| self.profile_flags(p));
                (profile.map(|p| self.template_name(TemplateName::SpliceProfile(p))), dep | pf)
            }
            TemplateName::SpliceProfile(p) => (None, dep | self.profile_flags(p)),
        };
        let flags = flags.without(TypeFlags::SUGAR);
        if let Some(id) = self.names.push(key, hash, flags.bits(), canon.map(TemplateNameId::index))
        {
            TemplateNameId::new(id)
        } else {
            self.exhausted = true;
            TemplateNameId::new(1)
        }
    }

    /// A template name for an unresolved set of function templates.
    pub fn overloaded_template_name(&mut self, decls: &[DeclId]) -> TemplateNameId {
        let start = self.name_decls.len();
        if decls.is_empty() {
            return self.template_name(TemplateName::Overloaded(DeclSet::EMPTY));
        }
        if start.checked_add(decls.len()).is_none_or(|e| e > u32::MAX as usize) {
            self.exhausted = true;
            return TemplateNameId::new(1);
        }
        self.name_decls.extend_from_slice(decls);
        let set = DeclSet { start: start as u32, len: decls.len() as u32 };
        self.template_name(TemplateName::Overloaded(set))
    }

    /// The declarations of an overload set.
    pub fn name_decls(&self, s: DeclSet) -> &[DeclId] {
        &self.name_decls[s.start as usize..(s.start + s.len) as usize]
    }

    /// The template name `n`.
    pub fn template_name_view(&self, n: TemplateNameId) -> TemplateName {
        decode(self.names.get(n.index()).key)
    }

    /// The canonical form of `n`.
    pub fn canonical_template_name(&self, n: TemplateNameId) -> TemplateNameId {
        TemplateNameId::new(self.names.get(n.index()).canon)
    }

    /// True if `a` and `b` name the same template.
    pub fn same_template_name(&self, a: TemplateNameId, b: TemplateNameId) -> bool {
        self.canonical_template_name(a) == self.canonical_template_name(b)
    }

    /// The template declaration `n` names, if it resolves to one.
    pub fn template_decl(&self, n: TemplateNameId) -> Option<DeclId> {
        match self.template_name_view(self.canonical_template_name(n)) {
            TemplateName::Template(d) => Some(d),
            _ => None,
        }
    }

    /// The dependence flags of `n`.
    pub fn template_name_flags(&self, n: TemplateNameId) -> TypeFlags {
        TypeFlags::from_bits(self.names.get(n.index()).flags)
    }
}
