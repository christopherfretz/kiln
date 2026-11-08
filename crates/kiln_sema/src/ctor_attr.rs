//! GNU `constructor` / `destructor` (spec/sema.md §4.5; clang
//! `handleConstructorAttr` / `handleDestructorAttr`, `AttrDocs`
//! "constructor, destructor"): a function run before `main` (after it,
//! for `destructor`) through `@llvm.global_ctors` / `@llvm.global_dtors`,
//! at the optional priority (65535 when none is written).
//!
//! `act_on_attribute` keeps the first argument and the argument count;
//! [`Sema::check_ctor_dtor_attrs`] checks them in clang's order (the
//! subject, then the count, then `checkUInt32Argument`) and replaces the
//! node with one holding the priority. A value-dependent priority is an
//! error under clang 19 and accepted by clang 22, which evaluates it at
//! instantiation ([`Sema::instantiate_ctor_dtor_priority`]).

use kiln_ast::attr::{AttrKind, AttrNode, AttrSyntax};
use kiln_ast::ids::RawId;
use kiln_ast::{AttrId, DeclId, ExprId};
use kiln_diag::DiagId;
use kiln_source::Loc;

use crate::Sema;
use crate::decl::AttrArg;

/// The priority of a `constructor` / `destructor` written without one
/// (clang `ConstructorAttr::DefaultPriority`).
pub(crate) const DEFAULT_PRIORITY: u32 = 65535;

impl Sema<'_, '_> {
    /// `constructor` / `destructor` as written: `a` = the raw `ExprId` of
    /// the first argument (0: none), `b` = the argument count.
    pub(crate) fn ctor_dtor_attribute(
        &mut self,
        kind: AttrKind,
        syntax: AttrSyntax,
        args: &[AttrArg],
        loc: Loc,
    ) -> AttrId {
        let a = match args.first() {
            Some(AttrArg::Expr(e)) => e.raw(),
            // Not an expression (cannot be written: the parser hands
            // every argument as one); an invalid argument.
            Some(_) => u32::MAX,
            None => 0,
        };
        let b = u32::try_from(args.len()).unwrap_or(u32::MAX);
        self.ast.attrs.push(AttrNode { kind, syntax, flags: 0, loc, a, b })
    }

    /// Checks the `constructor` / `destructor` attributes among `attrs`
    /// of declaration `d`, dropping the invalid ones and replacing each
    /// valid one by a node whose `a` is the priority.
    pub(crate) fn check_ctor_dtor_attrs(&mut self, d: DeclId, attrs: &mut Vec<AttrId>) {
        let mut i = 0;
        while i < attrs.len() {
            let node = *self.ast.attrs.get(attrs[i]);
            if !matches!(node.kind, AttrKind::CONSTRUCTOR | AttrKind::DESTRUCTOR) {
                i += 1;
                continue;
            }
            match self.ctor_dtor_priority(d, node) {
                Some(n) => {
                    attrs[i] = self.ast.attrs.push(n);
                    i += 1;
                }
                None => {
                    attrs.remove(i);
                }
            }
        }
    }

    /// The checked node of `constructor` / `destructor` attribute `node`
    /// on `d`, or `None` after a diagnostic.
    fn ctor_dtor_priority(&mut self, d: DeclId, node: AttrNode) -> Option<AttrNode> {
        let canonical =
            if node.kind == AttrKind::CONSTRUCTOR { "constructor" } else { "destructor" };
        let name = format!("'{}'", self.attr_shown(&node, canonical));
        // `SubjectList<[Function]>`, a warning.
        if !self.ast.decls.get(d).kind().is_function() {
            let msg = self
                .diag(DiagId::sema_attribute_wrong_decl_type, node.loc)
                .arg(name)
                .arg("functions");
            self.emit(msg);
            return None;
        }
        if node.b > 1 {
            let msg = self.diag(DiagId::sema_attribute_too_many_arguments_one, node.loc).arg(name);
            self.emit(msg);
            return None;
        }
        if node.b == 0 {
            return Some(AttrNode { a: DEFAULT_PRIORITY, b: 0, ..node });
        }
        let Some(e) = ExprId::from_raw(node.a).filter(|_| node.a != u32::MAX) else {
            let msg =
                self.diag(DiagId::sema_attribute_requires_integer_constant, node.loc).arg(name);
            self.emit(msg);
            return None;
        };
        let x = self.ast.exprs.get(e);
        if self.clang_compat >= 22 && (x.is_value_dependent() || x.is_type_dependent()) {
            let flags = node.flags | AttrNode::PRIORITY_DEPENDENT;
            return Some(AttrNode { flags, b: 0, ..node });
        }
        let p = self.uint32_attr_arg(e, &name, node.loc)?;
        Some(AttrNode { a: p, b: 0, ..node })
    }

    /// The node of dependent-priority `constructor` / `destructor`
    /// attribute `node` whose priority, substituted, is `e`: the checked
    /// priority, or `None` after a diagnostic.
    pub(crate) fn instantiate_ctor_dtor_priority(
        &mut self,
        node: AttrNode,
        e: ExprId,
    ) -> Option<AttrNode> {
        let canonical =
            if node.kind == AttrKind::CONSTRUCTOR { "constructor" } else { "destructor" };
        let name = format!("'{}'", self.attr_shown(&node, canonical));
        let p = self.uint32_attr_arg(e, &name, node.loc)?;
        let flags = node.flags & !AttrNode::PRIORITY_DEPENDENT;
        Some(AttrNode { flags, a: p, b: 0, ..node })
    }
}
