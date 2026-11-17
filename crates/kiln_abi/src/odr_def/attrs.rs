//! The listed attributes of O-10 (K-3e): the closed list of scheme 1, as
//! WRITTEN on the definition and its redeclaration chain (for an
//! instantiation, on its pattern's chain), never their effective value: an
//! implicit attribute (`#pragma GCC visibility push`, `#pragma clang
//! attribute`, `-fvisibility=`) and an inherited copy are not written.
//!
//! The chain's attributes are a SET: each one serialized on its own
//! (kind, then its arguments), the records sorted and deduplicated, so a
//! redeclaration that repeats an attribute, or a unit that sees one more
//! redeclaration repeating it, hashes alike. `[[indeterminate]]` is a
//! parameter's: a bit per parameter, set when any declaration of the chain
//! writes it on that parameter.
//!
//! Arguments: `alignas(e)` / `aligned(e)` the expression (a dependent one
//! by structure, the signature area), `alignas(T)` the type, `aligned`
//! without one a token of its own; `target("s")` and `target_version("s")`
//! the string's bytes. The spelling (`alignas` against `gnu::aligned`,
//! `[[noreturn]]` against `[[gnu::noreturn]]`) is not hashed: the
//! arguments decide the meaning. `target_clones` keeps no arguments in the
//! AST: it hashes as present and makes the row partial.
//!
//! Not on the list and so not hashed (O-10): every other attribute, among
//! them `visibility`, `exclude_from_explicit_instantiation`, `abi_tag`,
//! `weak`, `section` (mortar's ruling of 11-17 03:11 ET), `deprecated`,
//! `nodiscard`, `maybe_unused`. The calling convention, GNU `noreturn` of
//! the function type and `regparm` are hashed from the type
//! (`declarator`). `packed`, `no_unique_address`, `trivial_abi` are a
//! class's (kind 3).

use kiln_ast::attr::{AttrKind, AttrNode};
use kiln_ast::decl::DeclFlags;
use kiln_ast::ids::ListId;
use kiln_ast::{AttrId, DeclId, ExprId, QualType, RawId, StringLitId};

use super::{Area, DefHasher, Item, Site, T_NONE, Work};

/// The listed attributes of a declaration chain.
const T_ATTRS: u8 = 0xBD;

/// The closed list of O-10 (scheme 1) that a declaration carries.
const LISTED: [AttrKind; 6] = [
    AttrKind::ALIGNED,
    AttrKind::NORETURN,
    AttrKind::TARGET,
    AttrKind::TARGET_CLONES,
    AttrKind::TARGET_VERSION,
    AttrKind::INDETERMINATE,
];

/// Not written: added by sema (bit 0) or inherited from a previous
/// declaration (bit 1).
const NOT_WRITTEN: u8 = AttrNode::IMPLICIT | 2;

impl DefHasher<'_> {
    /// The written attributes of `d` itself.
    fn written(&self, d: DeclId) -> impl Iterator<Item = AttrNode> + '_ {
        let ast = self.ast;
        let list = if ast.decls.get(d).has(DeclFlags::HAS_ATTRS) {
            ast.attrs.of_decl(d)
        } else {
            ListId::EMPTY
        };
        ast.lists
            .iter::<AttrId>(list)
            .map(|a| *ast.attrs.get(a))
            .filter(|n| n.flags & NOT_WRITTEN == 0)
    }

    /// The chain whose attributes `def` hashes: its own, or for an
    /// instantiation its pattern's (an instantiated copy is not written).
    fn attr_chain(&self, def: DeclId) -> Vec<DeclId> {
        let ast = self.ast;
        let from = ast.instantiated_from(ast.first(def)).unwrap_or(def);
        ast.redecls_of(from).collect()
    }

    /// The listed attributes of `def`'s chain (module docs); with
    /// `params`, `def` is a function and `[[indeterminate]]` is read per
    /// parameter.
    pub(super) fn listed_attrs(&mut self, def: DeclId, params: bool) {
        let ast = self.ast;
        if !LISTED.iter().any(|&k| ast.attrs.has_kind(k)) {
            return self.byte(T_NONE);
        }
        let chain = self.attr_chain(def);
        let nodes: Vec<AttrNode> = chain
            .iter()
            .flat_map(|&r| self.written(r).collect::<Vec<_>>())
            .filter(|n| LISTED.contains(&n.kind) && n.kind != AttrKind::INDETERMINATE)
            .collect();
        let mut records: Vec<Vec<u8>> = Vec::with_capacity(nodes.len());
        for n in nodes {
            let start = self.out.len();
            self.attr_record(n);
            records.push(self.out.split_off(start));
        }
        records.sort_unstable();
        records.dedup();
        self.byte(T_ATTRS);
        self.count(records.len());
        for r in &records {
            self.out.extend_from_slice(r);
        }
        // `[[indeterminate]]`: one bit per parameter.
        let mut indet: Vec<u32> = Vec::new();
        for &r in chain.iter().filter(|_| params) {
            let Some(f) = ast.decls.function(r) else { continue };
            for (i, p) in ast.lists.iter::<DeclId>(f.params).enumerate() {
                let i = u32::try_from(i).unwrap_or(u32::MAX);
                if self.written(p).any(|n| n.kind == AttrKind::INDETERMINATE) && !indet.contains(&i)
                {
                    indet.push(i);
                }
            }
        }
        if !params && self.written(def).any(|n| n.kind == AttrKind::INDETERMINATE) {
            indet.push(0);
        }
        indet.sort_unstable();
        self.count(indet.len());
        for i in indet {
            self.u32(i);
        }
    }

    /// One attribute: its kind and its arguments.
    fn attr_record(&mut self, n: AttrNode) {
        self.u32(u32::from(n.kind.0));
        match n.kind {
            AttrKind::ALIGNED => {
                if let Some(e) = ExprId::from_raw(n.a) {
                    self.byte(1);
                    self.attr_expr(e);
                } else if let Some(t) = QualType::from_raw(n.b) {
                    self.byte(2);
                    self.ty(t);
                } else {
                    self.byte(0);
                }
            }
            AttrKind::TARGET | AttrKind::TARGET_VERSION => match StringLitId::from_raw(n.a) {
                Some(s) => {
                    let b = self.ast.strings.bytes(s);
                    self.count(b.len());
                    self.out.extend_from_slice(b);
                }
                None => self.byte(T_NONE),
            },
            // The clone list is not kept: present, and partial.
            AttrKind::TARGET_CLONES => self.partial(Area::Attrs, Site::Item(Item::Attributes)),
            _ => {}
        }
    }

    /// An attribute's argument expression, now (a dependent one, on a
    /// pattern, by structure: the expression path's dependent arms).
    fn attr_expr(&mut self, e: ExprId) {
        let base = self.stack.len();
        self.stack.push(Work::Expr(e));
        self.run_nested(base);
    }

    /// The `[[assume(e)]]` attributes of attributed statement kind `k`, in
    /// order (each hashes `e`: it changes what is undefined); nothing
    /// without one.
    pub(super) fn assume_attrs(&mut self, k: kiln_ast::stmt::StmtKind, list: ListId) {
        let ast = self.ast;
        let es: Vec<ExprId> = ast
            .lists
            .iter::<AttrId>(list)
            .map(|a| *ast.attrs.get(a))
            .filter(|n| n.kind == AttrKind::ASSUME)
            .filter_map(|n| ExprId::from_raw(n.a))
            .collect();
        if es.is_empty() {
            return;
        }
        self.stmt_tag(k);
        self.count(es.len());
        for e in es {
            self.attr_expr(e);
        }
    }
}
