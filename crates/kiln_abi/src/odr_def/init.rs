//! Initialization and object expressions in the definition hash (K-3c):
//! construct expressions by resolved constructor, initializer lists in
//! their semantic form, default arguments and default member
//! initializers AT THE USE, `new`/`delete` by resolved operator
//! functions, traits, `typeid`, `throw`; and the parts of a definition
//! that only constructors and destructors have (the ctor-initializers in
//! initialization order, the deallocation function a virtual destructor
//! selects).
//!
//! Choices O-10 leaves open (spec/abi.md "Definition hash", the init
//! rows of the table):
//! - an ELIDABLE copy or move ([`kiln_ast::Ast::elidable_copy_source`])
//!   is transparent, its source hashes in its place: `return T();` at
//!   C++14 (a move from a temporary) equals C++17 (no copy);
//! - temporaries, their materialization, bind-temporary and cleanups
//!   stay transparent (K-3b): they follow from the operands;
//! - an initializer list hashes its SEMANTIC form: its type, the union
//!   member it initializes, every element in field order (a designated
//!   initializer is its resolved field's position; skipped fields are
//!   their value-initialization), the array filler;
//! - a default argument and a default member initializer hash as their
//!   expression where they are used (the caller's definition holds them);
//! - a type trait and an array trait hash their trait, types and value;
//!   `noexcept(e)` its operand only (its value reads exception
//!   specifications that may be computed lazily).

use kiln_ast::decl::CtorInitKind;
use kiln_ast::expr::{ExprFlags, ExprKind as K, TypeidOperand};
use kiln_ast::ids::ListId;
use kiln_ast::{DeclId, ExprId, QualType, TypeId};

use super::{Area, DefHasher, Site, T_FIELD, T_NONE, Work};

/// A ctor-initializer: kind byte, target, initializer.
pub(super) const T_CTOR_INIT: u8 = 0xAC;
/// A destructor's deallocation function (an entity item follows).
pub(super) const T_DEALLOC: u8 = 0xAE;

impl DefHasher<'_> {
    /// An optional entity reference: the entity, or [`T_NONE`].
    fn opt_decl_ref(&mut self, d: Option<DeclId>) {
        match d {
            Some(d) => self.decl_ref(d),
            None => self.byte(T_NONE),
        }
    }

    /// The union member an initializer initializes, or [`T_NONE`].
    fn union_member(&mut self, d: Option<DeclId>) {
        match d {
            Some(f) => {
                self.byte(T_FIELD);
                self.owner(f);
                self.name(f);
            }
            None => self.byte(T_NONE),
        }
    }

    /// The node kinds of the init area (every arm a kind of
    /// [`DefHasher::expr`]'s init group).
    #[allow(clippy::too_many_lines, reason = "one arm per expression kind")]
    pub(super) fn init_expr(&mut self, e: ExprId, k: K) {
        let ast = self.ast;
        let xs = &ast.exprs;
        let n = *xs.get(e);
        match k {
            K::CXXConstructExpr | K::CXXTemporaryObjectExpr => {
                if let Some(src) = ast.elidable_copy_source(e) {
                    // The copy is omitted: what it copied stands here.
                    return self.stack.push(Work::Expr(src));
                }
                self.expr_tag(k);
                let info = xs.construct_info(e);
                // Not `immediate_escalating`: computed per use.
                self.byte(
                    info.kind as u8
                        | u8::from(info.list) << 2
                        | u8::from(info.std_init_list) << 3
                        | u8::from(info.zeroing) << 4,
                );
                self.ty(n.ty());
                self.decl_ref(xs.constructor(e));
                self.exprs(xs.construct_args(e));
            }
            K::CXXInheritedCtorInitExpr => {
                self.expr_tag(k);
                let (kind, virt) = xs.inherited_ctor_info(e);
                self.byte(kind as u8 | u8::from(virt) << 2);
                self.decl_ref(xs.constructor(e));
            }
            K::InitListExpr => {
                self.expr_tag(k);
                self.ty(n.ty());
                let extra = *xs.init_list(e);
                self.union_member(extra.union_field);
                self.exprs(xs.init_list_inits(e));
                self.stack.push(Work::OptExpr(extra.array_filler));
            }
            K::CXXParenListInitExpr => {
                self.expr_tag(k);
                self.ty(n.ty());
                self.union_member(xs.paren_list_union_field(e));
                self.exprs(xs.paren_list_inits(e));
                self.stack.push(Work::OptExpr(xs.paren_list_filler(e)));
            }
            K::ImplicitValueInitExpr | K::CXXScalarValueInitExpr => {
                self.expr_tag(k);
                self.ty(n.ty());
            }
            K::NoInitExpr | K::ArrayInitIndexExpr => self.expr_tag(k),
            K::ArrayInitLoopExpr => {
                self.expr_tag(k);
                let [common, elem] = xs.array_init_loop(e);
                self.stack.extend([Work::Expr(common), Work::Expr(elem)]);
            }
            K::DesignatedInitUpdateExpr => {
                self.expr_tag(k);
                let [base, update] = xs.designated_init_update(e);
                self.stack.extend([Work::Expr(base), Work::Expr(update)]);
            }
            K::CXXStdInitializerListExpr | K::CompoundLiteralExpr => {
                self.expr_tag(k);
                self.ty(n.ty());
                let op = if k == K::CompoundLiteralExpr {
                    Some(xs.compound_literal_init(e))
                } else {
                    ExprId::try_new(n.a())
                };
                self.stack.push(Work::OptExpr(op));
            }
            // At the use: the default argument's or default member
            // initializer's expression is part of this definition.
            K::CXXDefaultArgExpr => {
                self.expr_tag(k);
                self.stack.push(Work::OptExpr(ast.default_arg_expr(e)));
            }
            K::CXXDefaultInitExpr => {
                self.expr_tag(k);
                self.stack.push(Work::OptExpr(ast.default_init_expr(e)));
            }
            K::CXXNewExpr => {
                self.expr_tag(k);
                let ne = *xs.new_extra(e);
                self.byte(
                    u8::from(n.has(ExprFlags::GLOBAL))
                        | u8::from(n.has(ExprFlags::ARRAY_FORM)) << 1
                        | (xs.new_init_style(e) as u8) << 2,
                );
                self.ty(ne.allocated_ty);
                self.opt_decl_ref(ne.operator_new);
                self.opt_decl_ref(ne.operator_delete);
                match ne.placement {
                    Some(p) => self.exprs(p),
                    None => self.byte(T_NONE),
                }
                self.stack.extend([Work::OptExpr(ne.array_size), Work::OptExpr(ne.init)]);
            }
            K::CXXDeleteExpr => {
                self.expr_tag(k);
                self.byte(
                    u8::from(n.has(ExprFlags::GLOBAL))
                        | u8::from(n.has(ExprFlags::ARRAY_FORM)) << 1,
                );
                self.opt_decl_ref(xs.operator_delete(e));
                self.stack.push(Work::Expr(xs.delete_operand(e)));
            }
            K::CXXThrowExpr => {
                self.expr_tag(k);
                self.stack.push(Work::OptExpr(xs.throw_operand(e)));
            }
            K::CXXNoexceptExpr => {
                self.expr_tag(k);
                self.stack.push(Work::Expr(xs.noexcept_operand(e)));
            }
            K::CXXTypeidExpr => {
                self.expr_tag(k);
                match xs.typeid_operand(e) {
                    TypeidOperand::Type(t) => {
                        self.byte(1);
                        self.ty(t);
                    }
                    TypeidOperand::Expr(o) => {
                        self.byte(0);
                        self.stack.push(Work::Expr(o));
                    }
                }
            }
            K::TypeTraitExpr => {
                self.expr_tag(k);
                self.byte(n.aux());
                self.byte(u8::from(xs.type_trait_value(e)));
                let l: ListId = xs.type_trait_args(e);
                self.count(ast.lists.len(l));
                for t in ast.lists.iter::<QualType>(l) {
                    self.ty(t);
                }
            }
            K::ArrayTypeTraitExpr => {
                self.expr_tag(k);
                self.byte(n.aux());
                self.ty(xs.array_trait_type(e));
                let t = xs.array_trait(e);
                self.u64(t.value);
                self.stack.push(Work::OptExpr(t.dim));
            }
            K::ExpressionTraitExpr => {
                self.expr_tag(k);
                self.byte(n.aux());
                self.byte(u8::from(xs.type_trait_value(e)));
                self.stack.push(Work::Expr(xs.expression_trait_operand(e)));
            }
            // Not an init kind ([`DefHasher::expr`] routes only those).
            _ => self.partial(Area::Init, Site::Expr(k)),
        }
    }

    /// A constructor's ctor-initializers in initialization order as sema
    /// completed them (virtual bases once, direct bases, members from
    /// their mem-initializer, default member initializer or the implicit
    /// rule; a delegating constructor's target call), each hashed with
    /// its whole initializer before the next.
    pub(super) fn ctor_inits(&mut self, def: DeclId) {
        let ast = self.ast;
        let inits = ast.ctor_inits(def);
        self.count(inits.len());
        for ci in inits {
            self.byte(T_CTOR_INIT);
            self.byte(ci.kind as u8);
            match ci.kind {
                CtorInitKind::Base | CtorInitKind::VirtualBase => {
                    self.ty(QualType::unqual(TypeId::new(ci.target)));
                }
                CtorInitKind::Field | CtorInitKind::IndirectField => {
                    let f = DeclId::new(ci.target);
                    self.byte(T_FIELD);
                    self.owner(f);
                    self.name(f);
                }
                CtorInitKind::Delegating => {}
            }
            self.stack.push(Work::Expr(ci.init));
            self.run();
        }
    }

    /// A destructor's deallocation function (sema records it for a
    /// virtual destructor, [class.dtor]/16; mortar's ruling 1): hashed
    /// into every variant alike.
    pub(super) fn dealloc(&mut self, first: DeclId) {
        match self.ast.decls.cold.dtor_delete.get(first).copied() {
            Some(op) => {
                self.byte(T_DEALLOC);
                self.decl_ref(op);
            }
            None => self.byte(T_NONE),
        }
    }
}
