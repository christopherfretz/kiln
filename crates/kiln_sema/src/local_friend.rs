//! Unqualified friend functions of local classes ([class.friend]/11):
//! the redeclaration lookup confined to the innermost enclosing block
//! scope, and clang's diagnostics when it finds nothing there
//! (`DiagnoseInvalidRedeclaration` with `IsLocalFriend`): near-match
//! notes for the declarations of an outer block, else a "did you mean"
//! for a namespace function found by typo correction
//! (spec/sema-class.md §2.4).

use kiln_ast::decl::{DeclKind, NamespaceData};
use kiln_ast::names::DeclName;
use kiln_ast::types::{ExcSpec, NoexceptValue, TypeView};
use kiln_ast::{DeclId, QualType};
use kiln_diag::DiagId;
use kiln_source::Loc;
use kiln_source::symbol::Symbol;

use crate::Sema;
use crate::decl::sym_str;
use crate::overload::redecl::NewFn;
use crate::scope::{Ns, ScopeKind};

/// The outcome of [`Sema::check_local_friend`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum LocalFriend {
    /// Not an unqualified friend of a local class.
    NotLocal,
    /// It redeclares this function of the innermost enclosing block scope.
    Redeclares(DeclId),
    /// It redeclares nothing; diagnosed.
    Invalid,
}

/// The outcome of [`Sema::local_friend_member`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum LocalFriendMember {
    /// No member of the name in the local classes enclosing the friend.
    None,
    /// It redeclares a member of this class (its definition).
    Member(DeclId),
    /// Members of the name were found, none of which it redeclares;
    /// diagnosed.
    Invalid,
}

/// How a function's parameters compare with the friend's (clang
/// `hasSimilarParameters`).
#[derive(Clone, Copy)]
enum Similar {
    /// All the same.
    Same,
    /// The first that differs: its index, its type in the function and
    /// in the friend.
    Differs(usize, QualType, QualType),
}

/// A function found by typo correction for a local friend: its name and
/// namespace, the qualifier it is suggested with, clang's normalized edit distance
/// (characters weighted 100, qualifier components 110), and the
/// declaration the note points at.
struct Correction {
    name: Symbol,
    ns: DeclId,
    qual: String,
    distance: usize,
    decl: DeclId,
}

impl Sema<'_, '_> {
    /// [class.friend]/11: an unqualified friend function `dname` declared
    /// in a local class must redeclare a function declared in the
    /// innermost enclosing non-class (block) scope. Otherwise "no
    /// matching function found in local scope", with clang's notes: each
    /// function of the nearest enclosing block that declares the name
    /// whose parameters are similar ("local declaration nearly matches",
    /// or the first parameter whose type differs), or, when no block
    /// declares it, the namespace function typo correction finds
    /// ([`Sema::local_friend_correction`]). An operator friend matches
    /// the blocks' operator declarations (`Scope.local_ops`) and gets no
    /// typo correction.
    pub(crate) fn check_local_friend(
        &mut self,
        dname: DeclName,
        new: NewFn,
        loc: Loc,
    ) -> LocalFriend {
        let Some(c) = self.current_class() else { return LocalFriend::NotLocal };
        if !self.is_local_class(c.record) {
            return LocalFriend::NotLocal;
        }
        let (name, op) = (dname.as_ident(), dname.as_operator());
        if name.is_none() && op.is_none() {
            self.error(DiagId::sema_no_matching_local_friend, loc);
            return LocalFriend::Invalid;
        }
        // The enclosing block scopes, innermost first, up to the
        // function's outermost one (clang `LookupLocalFriendName`).
        let mut blocks = self
            .scopes
            .iter()
            .rev()
            .skip_while(|s| s.is_transparent() || s.kind == ScopeKind::Record)
            .filter(|s| !s.is_transparent());
        let mut found: Vec<DeclId> = Vec::new();
        let mut innermost = true;
        for s in blocks.by_ref() {
            if !s.is_local() {
                break;
            }
            match (name, op) {
                (Some(name), _) => found.extend(
                    s.locals
                        .iter()
                        .filter(|&&(n, _, k)| n == name && k == Ns::Ordinary)
                        .map(|e| e.1),
                ),
                (None, Some(op)) => {
                    found.extend(s.local_ops.iter().filter(|e| e.0 == op).map(|e| e.1));
                }
                (None, None) => {}
            }
            if !found.is_empty() || s.kind == ScopeKind::Function {
                break;
            }
            innermost = false;
        }
        let fns: Vec<DeclId> = found
            .iter()
            .copied()
            .filter(|&x| {
                let k = self.kind_of(x);
                k.payload() == kiln_ast::decl::PayloadClass::Function
                    || k == DeclKind::FunctionTemplate
            })
            .collect();
        if innermost && let Some(p) = self.pick_prev_function(&fns, new) {
            return LocalFriend::Redeclares(p);
        }
        let msg = if found.is_empty()
            && let Some(name) = name
        {
            match self.local_friend_correction(name, new) {
                Some(c) => {
                    let qual = format!("{}{}", c.qual, sym_str(self, c.name));
                    let note = self
                        .diag(DiagId::note_entity_declared_here, self.decl_loc(c.decl))
                        .arg(format!("'{qual}'"));
                    self.diag(DiagId::sema_no_matching_local_friend_suggest, loc)
                        .arg(sym_str(self, name))
                        .arg(qual)
                        .with_note(note)
                }
                None => self.diag(DiagId::sema_no_matching_local_friend, loc),
            }
        } else if found.is_empty() {
            // An operator: typo correction suggests nothing.
            self.diag(DiagId::sema_no_matching_local_friend, loc)
        } else {
            let msg = self.diag(DiagId::sema_no_matching_local_friend, loc);
            self.local_friend_near_matches(msg, fns, new)
        };
        self.emit(msg);
        LocalFriend::Invalid
    }

    /// Clang's near-match notes on "no matching function found in local
    /// scope": each function of `fns` whose parameters are similar to the
    /// friend's gets "local declaration nearly matches", or the first
    /// parameter whose type differs.
    fn local_friend_near_matches(
        &mut self,
        mut msg: kiln_diag::Diag,
        fns: Vec<DeclId>,
        new: NewFn,
    ) -> kiln_diag::Diag {
        for f in fns {
            if self.kind_of(f) == DeclKind::FunctionTemplate {
                continue;
            }
            let Some(similar) = self.similar_params(f, new.ty) else { continue };
            let note = match similar {
                Similar::Same => self.diag(DiagId::note_local_decl_close_match, self.decl_loc(f)),
                Similar::Differs(i, local, friend) => {
                    let params: Vec<DeclId> = self
                        .ast
                        .decls
                        .function(f)
                        .map(|fd| self.ast.lists.iter::<DeclId>(fd.params).collect())
                        .unwrap_or_default();
                    // The first type-specifier (clang `getTypeSpecStartLoc`).
                    let ploc = params.get(i).map_or(self.decl_loc(f), |&p| {
                        let begin = self.ast.decls.cold.begin_loc.get(p).copied();
                        self.type_spec_locs.get(&p).copied().or(begin).unwrap_or(self.decl_loc(p))
                    });
                    self.diag(DiagId::note_local_decl_close_param_match, ploc)
                        .arg(crate::expr::ordinal(i + 1))
                        .arg(self.ty_arg(local))
                        .arg(self.ty_arg(friend))
                }
            };
            msg = msg.with_note(note);
        }
        msg
    }

    /// [class.friend]/11 with [class.local]: the redeclaration lookup of
    /// an unqualified friend function `dname` of type `ty` declared in a
    /// local class first searches the class itself and the local classes
    /// enclosing it, innermost first, up to the block holding the
    /// outermost one (clang `LookupLocalFriendName`, a redeclaration
    /// lookup: members of base classes are not found). The first class
    /// declaring the name decides: a member function the friend
    /// corresponds to is redeclared, as if the friend were qualified by
    /// that class; a non-function member gives `redefinition of 'x' as
    /// different kind of symbol`; otherwise `no matching function found
    /// in local scope` (no notes, as in clang).
    pub(crate) fn local_friend_member(
        &mut self,
        dname: DeclName,
        ty: QualType,
        requires: Option<kiln_ast::ExprId>,
        loc: Loc,
    ) -> LocalFriendMember {
        let Some(c) = self.current_class() else { return LocalFriendMember::None };
        if !self.is_local_class(c.record) {
            return LocalFriendMember::None;
        }
        let classes: Vec<DeclId> = self
            .scopes
            .iter()
            .rev()
            .filter(|s| !s.is_transparent())
            .take_while(|s| s.kind == ScopeKind::Record)
            .filter_map(|s| s.entity)
            .collect();
        for class in classes {
            let def = self.ast.definition(class).unwrap_or(class);
            let found = self.lookup_in_dn(def, dname, crate::lookup::LookupKind::Ordinary).decls;
            if found.is_empty() {
                continue;
            }
            let new = NewFn {
                ty,
                is_static: false,
                head: None,
                requires,
                enable_if: kiln_ast::ids::ListId::EMPTY,
                member_using: false,
                pass_object_size: false,
            };
            let Some(p) = self.pick_prev_function(&found, new) else {
                self.error(DiagId::sema_no_matching_local_friend, loc);
                return LocalFriendMember::Invalid;
            };
            let k = self.kind_of(p);
            if k.payload() != kiln_ast::decl::PayloadClass::Function
                && k != DeclKind::FunctionTemplate
            {
                let name = self.decl_name_str(dname);
                let msg = self.diag(DiagId::sema_redefinition_different_kind, loc).arg(name);
                let msg = self.note_prev(msg, p, true);
                self.emit(msg);
                return LocalFriendMember::Invalid;
            }
            return LocalFriendMember::Member(def);
        }
        LocalFriendMember::None
    }

    /// clang's `hasSimilarParameters`: whether function `f` has as many
    /// parameters as function type `ty`, each of the same type ignoring
    /// top-level cv-qualifiers, or of the same type once pointers and
    /// references are stripped from both, or naming classes or
    /// enumerations of the same name.
    fn similar_params(&self, f: DeclId, ty: QualType) -> Option<Similar> {
        let t = &self.ast.types;
        let fty = self.ast.decls.get(f).ty()?;
        // The types as written, which the note prints.
        let written = |q: QualType| t.proto_of(q.ty()).or_else(|| t.proto(q));
        let (pa, pb) = (written(fty)?, written(ty)?);
        let (a, b) = (t.params(pa), t.params(pb));
        if a.len() != b.len() {
            return None;
        }
        let core = |mut q: QualType| {
            for _ in 0..64 {
                match t.canonical_view(q) {
                    TypeView::Pointer(p) | TypeView::LValueRef(p) | TypeView::RValueRef(p) => q = p,
                    _ => break,
                }
            }
            q
        };
        let tag_name = |q: QualType| {
            t.as_record(q)
                .or_else(|| t.as_enum(q))
                .and_then(|d| self.ast.decls.get(d).name().as_ident())
        };
        let mut first = None;
        for (i, (&x, &y)) in a.iter().zip(b).enumerate() {
            if t.same_unqualified(x, y) {
                continue;
            }
            let (cx, cy) = (core(x), core(y));
            if !t.same_unqualified(cx, cy)
                && (tag_name(cx).is_none() || tag_name(cx) != tag_name(cy))
            {
                return None;
            }
            first.get_or_insert(Similar::Differs(i, x, y));
        }
        Some(first.unwrap_or(Similar::Same))
    }

    /// clang's typo correction for an invalid local friend `name`
    /// (`CorrectTypo` with `DifferentNameValidatorCCC`, then the retried
    /// redeclaration): among the namespace-scope functions (not
    /// templates, hidden friends or block-scope declarations) with no
    /// definition and similar parameters whose names are within a third
    /// of `name`'s length in edits, the one at the least normalized
    /// distance, qualified from the enclosing namespace. Functions of
    /// distinct names or namespaces tied for the least give nothing; so does a best candidate that
    /// would not be redeclared (another return type or parameter list, or
    /// a non-throwing exception specification against a potentially
    /// throwing one).
    fn local_friend_correction(&mut self, name: Symbol, new: NewFn) -> Option<Correction> {
        let typo = sym_str(self, name);
        let len = typo.len();
        let start = self.enclosing_namespace()?;
        let cur = self.named_ns_chain(start);
        let mut root = start;
        while let Some(p) = self.ast.decls.get(root).ctx() {
            root = p;
        }
        let mut cands: Vec<(DeclId, DeclId)> = Vec::new();
        let mut stack = vec![(root, root)];
        while let Some((ctx, ns)) = stack.pop() {
            for m in self.ast.members(ctx) {
                match self.kind_of(m) {
                    DeclKind::Namespace => stack.push((m, m)),
                    DeclKind::LinkageSpec | DeclKind::Export => stack.push((m, ns)),
                    DeclKind::Function => cands.push((m, ns)),
                    _ => {}
                }
            }
        }
        let mut best: Vec<Correction> = Vec::new();
        for (f, ns) in cands {
            let Some(cname) = self.ast.decls.get(f).name().as_ident() else { continue };
            let spelled = sym_str(self, cname);
            let ed = edit_distance(&typo, &spelled);
            if ed > len.div_ceil(3) || (len < 3 && cname != name) || (ed > 0 && len / ed < 3) {
                continue;
            }
            if self.ast.definition(f).is_some() || self.similar_params(f, new.ty).is_none() {
                continue;
            }
            let (qual, qd) = self.correction_qualifier(ns, &cur);
            let distance = (100 * ed + 110 * qd + 50) / 100;
            if cname != name && distance > 0 && len / distance < 3 {
                continue;
            }
            let f = self.ast.latest(f);
            let ns = self.ast.first(ns);
            if !best.iter().any(|c| c.name == cname && c.ns == ns) {
                best.push(Correction { name: cname, ns, qual, distance, decl: f });
            }
        }
        // One name in one namespace, the overloads of which are retried.
        let least = best.iter().map(|c| c.distance).min()?;
        let mut at_least = best.into_iter().filter(|c| c.distance == least);
        let c = at_least.next()?;
        if at_least.next().is_some() {
            return None;
        }
        // The retried redeclaration (errors trapped).
        let fty = self.ast.decls.get(c.decl).ty()?;
        if !self.same_ignoring_exc_spec(fty, new.ty) {
            return None;
        }
        let unknown = |e: ExcSpec| {
            matches!(
                e,
                ExcSpec::Deferred(_)
                    | ExcSpec::DependentNoexcept(_)
                    | ExcSpec::NoexceptExpr { value: NoexceptValue::Dependent(_), .. }
            )
        };
        let (pe, ne) = (self.written_exc_spec(fty), self.written_exc_spec(new.ty));
        (unknown(pe) || unknown(ne) || pe.is_nothrow() == ne.is_nothrow()).then_some(c)
    }

    /// The named, non-inline namespaces enclosing namespace `ns` (itself
    /// included), outermost first; the translation unit is not one.
    pub(crate) fn named_ns_chain(&self, mut ns: DeclId) -> Vec<DeclId> {
        let mut chain = Vec::new();
        for _ in 0..10_000 {
            if self.kind_of(ns) == DeclKind::Namespace
                && self.ast.decls.namespace(ns).is_some_and(|n| {
                    n.flags & (NamespaceData::INLINE | NamespaceData::ANONYMOUS) == 0
                })
            {
                chain.push(self.ast.first(ns));
            }
            match self.ast.decls.get(ns).ctx() {
                Some(p) => ns = p,
                None => break,
            }
        }
        chain.reverse();
        chain
    }

    /// The qualifier typo correction spells a function of namespace `ns`
    /// with, seen from the namespaces `cur` (clang's
    /// `NamespaceSpecifierSet::addNameSpecifier`), and its component
    /// count: `::` for the translation unit (counted as one); `::A::B::`
    /// for an enclosing namespace or when the first component would name
    /// an enclosing one; otherwise the path from the innermost common
    /// namespace. Inline and unnamed namespaces are not spelled.
    pub(crate) fn correction_qualifier(&self, ns: DeclId, cur: &[DeclId]) -> (String, usize) {
        let chain = self.named_ns_chain(ns);
        let common = chain.iter().zip(cur).take_while(|(a, b)| a == b).count();
        let spell = |s: &Self, part: &[DeclId]| {
            let mut out = String::new();
            for &n in part {
                out.push_str(&s.decl_name(n));
                out.push_str("::");
            }
            out
        };
        if chain.is_empty() {
            return ("::".to_owned(), 1);
        }
        let rel = &chain[common..];
        let global = rel.first().is_none_or(|&r| {
            let n = self.ast.decls.get(r).name();
            cur.iter().any(|&c| self.ast.decls.get(c).name() == n)
        });
        if global {
            (format!("::{}", spell(self, &chain)), chain.len())
        } else {
            (spell(self, rel), rel.len())
        }
    }
}

/// Levenshtein distance between `a` and `b` (substitutions allowed), by
/// byte as clang's `StringRef::edit_distance`.
pub(crate) fn edit_distance(a: &str, b: &str) -> usize {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, &ca) in a.iter().enumerate() {
        let mut diag = row[0];
        row[0] = i + 1;
        for (j, &cb) in b.iter().enumerate() {
            let next = (row[j + 1] + 1).min(row[j] + 1).min(diag + usize::from(ca != cb));
            diag = row[j + 1];
            row[j + 1] = next;
        }
    }
    row[b.len()]
}
