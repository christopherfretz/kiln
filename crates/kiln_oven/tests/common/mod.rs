//! The Oven's test DSL: C-subset source parsed straight into a kiln AST
//! (`parse`), a `SemaBridge` over it, and a harness that evaluates every
//! `static_assert` and namespace-scope constant initializer in order.

#![allow(dead_code, reason = "each test binary uses a different subset of the harness")]
#![allow(
    clippy::many_single_char_names,
    clippy::struct_excessive_bools,
    clippy::match_same_arms,
    clippy::too_many_lines,
    clippy::while_let_loop,
    clippy::collapsible_if,
    clippy::needless_pass_by_value,
    clippy::if_same_then_else,
    clippy::single_match_else,
    clippy::blocks_in_conditions,
    clippy::manual_assert,
    clippy::manual_let_else,
    clippy::wrong_self_convention,
    clippy::format_collect,
    reason = "test-only recursive-descent DSL: short names mirror the grammar, shape follows the C++ grammar"
)]

mod lex;
pub mod parse;

use std::collections::HashMap;

use kiln_abi::Target;
use kiln_ast::consts::{ConstStore, Scalar};
use kiln_ast::types::RecordLayout;
use kiln_ast::{Ast, ConstValueId, DeclId, ExprId, StmtId};
use kiln_diag::Diag;
use kiln_oven::{
    Budget, EvalContext, EvalFailure, EvalKind, FailKind, Oven, SemaBridge, VTableLayout,
};
use kiln_source::Loc;

pub use parse::Item;

/// The test `SemaBridge`: bodies are already parsed; layouts come from
/// `kiln_abi`.
pub struct Bridge<'a> {
    pub ast: &'a mut Ast,
    pub target: &'a Target,
    vt: VTableLayout,
}

impl<'a> Bridge<'a> {
    pub fn new(ast: &'a mut Ast, target: &'a Target) -> Self {
        Bridge { ast, target, vt: VTableLayout::default() }
    }
}

impl SemaBridge for Bridge<'_> {
    fn ast(&self) -> &Ast {
        self.ast
    }
    fn ensure_body(&mut self, f: DeclId) -> Option<StmtId> {
        let d = self.ast.definition(f)?;
        self.ast.decls.function(d)?.body
    }
    fn ensure_layout(&mut self, record: DeclId) -> &RecordLayout {
        kiln_abi::ensure_layout(&mut *self.ast, record, self.target)
    }
    fn vtable(&mut self, _record: DeclId) -> &VTableLayout {
        &self.vt
    }
    fn consts(&mut self) -> &mut ConstStore {
        &mut self.ast.consts
    }
    fn diag(&mut self, _d: Diag) {}
}

/// The x86-64 Linux target every test uses unless it says otherwise.
pub fn target() -> Target {
    kiln_abi::target("x86_64-unknown-linux-gnu").expect("target")
}

/// A parsed translation unit.
pub struct Tu {
    pub ast: Ast,
    pub target: Target,
    pub src: String,
    pub items: Vec<Item>,
    pub names: HashMap<String, DeclId>,
}

/// The first failed item of a run.
#[derive(Debug)]
pub struct Failed {
    pub kind: FailKind,
    /// `line:col: msg` lines: the diagnostic, then the oven's notes.
    pub text: String,
}

impl Tu {
    pub fn new(src: &str) -> Tu {
        Tu::with_target(src, target())
    }

    pub fn with_target(src: &str, target: Target) -> Tu {
        let p = parse::parse(src, &target);
        Tu { ast: p.ast, target, src: src.to_owned(), items: p.items, names: p.names }
    }

    pub fn bridge(&mut self) -> Bridge<'_> {
        Bridge::new(&mut self.ast, &self.target)
    }

    /// `line:col` of a location (1-based).
    pub fn line_col(&self, loc: Loc) -> String {
        let off = loc.to_raw().saturating_sub(1) as usize;
        let before = &self.src[..off.min(self.src.len())];
        let line = before.matches('\n').count() + 1;
        let col = off - before.rfind('\n').map_or(0, |i| i + 1) + 1;
        format!("{line}:{col}")
    }

    pub fn render(&self, f: &EvalFailure) -> String {
        f.notes.iter().map(|n| format!("{}: {}\n", self.line_col(n.loc), n.msg)).collect()
    }

    /// Evaluates every item in order; stops at the first failure.
    pub fn run(&mut self, oven: &mut Oven) -> Result<(), Failed> {
        let items = self.items.clone();
        for it in items {
            match it {
                Item::Assert { cond, loc } => {
                    let ctx = EvalContext::new(EvalKind::StaticAssert, loc);
                    let r = oven.evaluate(&mut self.bridge(), cond, ctx);
                    match r {
                        Ok(v) => {
                            if !self.truth(v) {
                                let text =
                                    format!("{}: static assertion failed\n", self.line_col(loc));
                                return Err(Failed { kind: FailKind::NotConstant, text });
                            }
                        }
                        Err(f) => {
                            let text = format!(
                                "{}: static assertion expression is not an integral constant expression\n{}",
                                self.line_col(loc),
                                self.render(&f)
                            );
                            return Err(Failed { kind: f.kind, text });
                        }
                    }
                }
                Item::Var { decl, init, loc, constexpr } => {
                    if self.ast.decls.begin_eval(decl).is_err() {
                        continue;
                    }
                    if constexpr {
                        let ctx = EvalContext::init(decl, loc);
                        match oven.evaluate(&mut self.bridge(), init, ctx) {
                            Ok(v) => self.ast.decls.finish_eval(decl, Some(v)),
                            Err(f) => {
                                self.ast.decls.finish_eval(decl, None);
                                let text = format!(
                                    "{}: constexpr variable must be initialized by a constant expression\n{}",
                                    self.line_col(loc),
                                    self.render(&f)
                                );
                                return Err(Failed { kind: f.kind, text });
                            }
                        }
                    } else {
                        let v = oven.try_fold(&mut self.bridge(), init);
                        self.ast.decls.finish_eval(decl, v);
                    }
                }
            }
        }
        Ok(())
    }

    fn truth(&self, v: ConstValueId) -> bool {
        match self.ast.consts.scalar(v) {
            Some(Scalar::Bool { value, .. }) => value,
            Some(Scalar::Int { bits, .. }) => bits.to_u128() != Some(0),
            s => panic!("static_assert condition is not a scalar: {s:?}"),
        }
    }

    /// The frozen value of a namespace-scope variable after `run`.
    pub fn value(&self, name: &str) -> ConstValueId {
        let d = self.names[name];
        let ast = &self.ast;
        let data = ast.decls.var(d).expect("variable");
        match data.eval.state() {
            kiln_ast::decl::VarEvalState::Value(v) => v,
            s => panic!("{name} has no constant value: {s:?}"),
        }
    }

    pub fn scalar(&self, name: &str) -> Scalar<'_> {
        self.ast.consts.scalar(self.value(name)).expect("scalar")
    }

    pub fn int(&self, name: &str) -> i128 {
        match self.scalar(name) {
            Scalar::Int { bits, signed: true, .. } => bits.to_i128().expect("i128"),
            Scalar::Int { bits, .. } => bits.to_u128().expect("u128").cast_signed(),
            Scalar::Bool { value, .. } => i128::from(value),
            s => panic!("{name} is not an integer: {s:?}"),
        }
    }

    pub fn float_bits(&self, name: &str) -> u128 {
        match self.scalar(name) {
            Scalar::Float { bits, .. } => bits,
            s => panic!("{name} is not floating: {s:?}"),
        }
    }

    /// Evaluates an expression already in the AST.
    pub fn eval(
        &mut self,
        oven: &mut Oven,
        e: ExprId,
        loc: Loc,
    ) -> Result<ConstValueId, EvalFailure> {
        oven.evaluate(&mut self.bridge(), e, EvalContext::new(EvalKind::StaticAssert, loc))
    }
}

/// Runs `src` with the default budget; panics with the rendered failure.
pub fn ok(src: &str) -> Tu {
    let mut tu = Tu::new(src);
    let mut oven = Oven::new(Budget::default());
    if let Err(f) = tu.run(&mut oven) {
        panic!("unexpected failure ({:?}):\n{}", f.kind, f.text);
    }
    tu
}

/// Runs `src`, which must fail; returns the rendered failure.
pub fn fails(src: &str) -> Failed {
    fails_with(src, Budget::default())
}

pub fn fails_with(src: &str, budget: Budget) -> Failed {
    let mut tu = Tu::new(src);
    let mut oven = Oven::new(budget);
    match tu.run(&mut oven) {
        Ok(()) => panic!("expected a failure:\n{src}"),
        Err(f) => f,
    }
}

/// Asserts that `src` fails with exactly `expected` (after trimming).
#[track_caller]
pub fn fails_exactly(src: &str, expected: &str) {
    let f = fails(src);
    assert_eq!(f.text.trim_end(), expected.trim(), "\nsource:\n{src}");
}

/// Asserts that `src` fails and the rendered failure contains `needle`.
#[track_caller]
pub fn fails_containing(src: &str, needle: &str) {
    let f = fails(src);
    assert!(f.text.contains(needle), "expected {needle:?} in:\n{}\nsource:\n{src}", f.text);
}
