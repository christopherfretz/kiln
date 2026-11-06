//! Trial-fold entry counts (feature `op-stats`, a measurement aid like the
//! opcode histogram in `vm/opstats.rs`; never built by default). Every
//! silent fold entry (`Oven::try_fold`, `fold_rvalue`, `fold_for_overflow`,
//! `is_constant_initialized`) is counted by its caller (the sema wrapper
//! and its call site, set by [`CallerGuard`]; none for codegen's direct
//! calls), its root expression kind, its shape (the first node outside the
//! leaf-fold domain, or `leaf`), whether a thunk or a function was compiled,
//! whether a call memo hit, and its result; repeats of an `ExprId` within
//! the translation unit are counted too. The table prints to stderr as
//! `trial-stats:` lines when the compiling thread exits (WP-656).

use std::cell::{Cell, RefCell};
use std::fmt::Write;
use std::panic::Location;

use kiln_ast::decl::DeclKind;
use kiln_ast::expr::{BinaryOp, CastKind, ExprKind, UnaryOp};
use kiln_ast::{Ast, DeclId, ExprId};
use kiln_source::fx::{FxHashMap, FxHashSet};

#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
struct Key {
    entry: &'static str,
    caller: &'static str,
    file: &'static str,
    root: &'static str,
    shape: &'static str,
    thunk: bool,
    func: bool,
    memo: bool,
    ok: bool,
}

#[derive(Default)]
struct Stats {
    rows: FxHashMap<Key, u64>,
    sites: FxHashMap<(&'static str, &'static str, u32), u64>,
    seen: FxHashMap<u32, &'static str>,
    repeats: u64,
    repeats_other_caller: u64,
    repeats_by_caller: FxHashMap<&'static str, u64>,
    thunks: u64,
    funcs: u64,
    funcs_outside: u64,
    depth: u32,
    no_eval: u64,
}

impl Drop for Stats {
    fn drop(&mut self) {
        let total: u64 = self.rows.values().sum();
        if total == 0 {
            return;
        }
        let mut s = format!(
            "trial-stats: entries={total} repeats={} repeats-other-caller={} \
             thunk-compiles={} func-compiles-in-trials={} func-compiles-elsewhere={} \
             no-eval={}\n",
            self.repeats,
            self.repeats_other_caller,
            self.thunks,
            self.funcs,
            self.funcs_outside,
            self.no_eval
        );
        let mut group = |name: &str, f: &dyn Fn(&Key) -> String| {
            let mut m: FxHashMap<String, [u64; 5]> = FxHashMap::default();
            for (k, &n) in &self.rows {
                let c = m.entry(f(k)).or_default();
                c[0] += n;
                c[1] += if k.thunk { n } else { 0 };
                c[2] += if k.func { n } else { 0 };
                c[3] += if k.memo { n } else { 0 };
                c[4] += if k.ok { n } else { 0 };
            }
            let mut v: Vec<_> = m.into_iter().collect();
            v.sort_by(|a, b| b.1[0].cmp(&a.1[0]).then_with(|| a.0.cmp(&b.0)));
            for (k, c) in v.iter().take(40) {
                let _ = writeln!(
                    s,
                    "trial-stats: {name} {k} n={} thunk={} func={} memo={} some={}",
                    c[0], c[1], c[2], c[3], c[4]
                );
            }
        };
        group("by-caller", &|k| format!("{}/{}", k.entry, k.caller));
        group("by-root", &|k| k.root.to_string());
        group("by-shape", &|k| k.shape.to_string());
        group("by-caller-shape", &|k| format!("{}/{} {}", k.entry, k.caller, k.shape));
        group("by-caller-root", &|k| format!("{}/{} {}", k.entry, k.caller, k.root));
        let mut v: Vec<_> = self.sites.iter().collect();
        v.sort_by(|a, b| b.1.cmp(a.1).then_with(|| a.0.cmp(b.0)));
        for ((w, f, l), n) in v.iter().take(30) {
            let _ = writeln!(s, "trial-stats: site {w} {f}:{l} n={n}");
        }
        let mut v: Vec<_> = self.repeats_by_caller.iter().collect();
        v.sort_by(|a, b| b.1.cmp(a.1));
        for (c, n) in v {
            let _ = writeln!(s, "trial-stats: repeat-by {c} n={n}");
        }
        eprint!("{s}");
    }
}

thread_local! {
    static STATS: RefCell<Stats> = RefCell::new(Stats::default());
    static CALLER: Cell<Option<(&'static str, &'static Location<'static>)>> =
        const { Cell::new(None) };
}

/// Sets the caller of the next fold entries while alive (the outermost
/// sema wrapper wins).
#[derive(Debug)]
pub struct CallerGuard {
    set: bool,
}

impl CallerGuard {
    /// Marks `wrapper` called from `at` as the caller, unless one is set.
    pub fn new(wrapper: &'static str, at: &'static Location<'static>) -> Self {
        let set = CALLER.with(|c| {
            if c.get().is_none() {
                c.set(Some((wrapper, at)));
                true
            } else {
                false
            }
        });
        CallerGuard { set }
    }
}

impl Drop for CallerGuard {
    fn drop(&mut self) {
        if self.set {
            CALLER.with(|c| c.set(None));
        }
    }
}

/// A thunk compile (a `compile_thunk` cache miss).
pub(crate) fn note_thunk() {
    STATS.with(|s| s.borrow_mut().thunks += 1);
}

/// A `compile_func` run.
pub(crate) fn note_func() {
    STATS.with(|s| {
        let mut s = s.borrow_mut();
        if s.depth > 0 {
            s.funcs += 1;
        } else {
            s.funcs_outside += 1;
        }
    });
}

/// An entry's state at its start.
pub(crate) struct Entry {
    root: &'static str,
    shape: &'static str,
    e: Option<ExprId>,
    thunks: u64,
    funcs: u64,
    memo: u64,
}

/// Starts counting a fold entry on `e`.
pub(crate) fn begin(ast: &Ast, e: Option<ExprId>, memo: u64) -> Entry {
    let (root, shape) = match e {
        Some(e) => (kind_name(ast.exprs.get(e).kind()), shape(ast, e)),
        None => ("-", "-"),
    };
    STATS.with(|s| {
        let mut s = s.borrow_mut();
        s.depth += 1;
        Entry { root, shape, e, thunks: s.thunks, funcs: s.funcs, memo }
    })
}

/// Ends the entry `t` of `entry`.
pub(crate) fn end(t: Entry, entry: &'static str, ok: bool, memo: u64) {
    let (caller, file, line) = match CALLER.with(Cell::get) {
        Some((w, at)) => (w, short(at.file()), at.line()),
        None => ("direct", "-", 0),
    };
    STATS.with(|s| {
        let mut s = s.borrow_mut();
        s.depth -= 1;
        let key = Key {
            entry,
            caller,
            file,
            root: t.root,
            shape: t.shape,
            thunk: s.thunks > t.thunks,
            func: s.funcs > t.funcs,
            memo: memo > t.memo,
            ok,
        };
        *s.rows.entry(key).or_default() += 1;
        *s.sites.entry((caller, file, line)).or_default() += 1;
        if let Some(e) = t.e {
            let tag: &'static str = caller;
            match s.seen.get(&e.index()).copied() {
                Some(first) => {
                    s.repeats += 1;
                    if first != tag || file == "-" {
                        s.repeats_other_caller += 1;
                    }
                    *s.repeats_by_caller.entry(tag).or_default() += 1;
                }
                None => {
                    s.seen.insert(e.index(), tag);
                }
            }
        }
    });
}

/// An entry that answered without evaluating (a cached variable state).
pub(crate) fn no_eval() {
    STATS.with(|s| s.borrow_mut().no_eval += 1);
}

fn short(f: &'static str) -> &'static str {
    let f = f.strip_prefix("crates/").unwrap_or(f);
    f.split_once("/src/").map_or(f, |(c, rest)| if c == "kiln_sema" { rest } else { f })
}

fn kind_name(k: ExprKind) -> &'static str {
    // `Debug` names leaked once per kind (a bounded set).
    thread_local! {
        static NAMES: RefCell<FxHashMap<ExprKind, &'static str>> = RefCell::new(FxHashMap::default());
    }
    NAMES.with(|m| {
        *m.borrow_mut().entry(k).or_insert_with(|| Box::leak(format!("{k:?}").into_boxed_str()))
    })
}

/// The first node of `e` (pre-order) outside the candidate leaf-fold
/// domain (integer literals, enumerators, `sizeof`/`alignof`, integral
/// casts, integer unary/binary/conditional operators), named by kind, or
/// `leaf`; a `DeclRefExpr` of a variable is `var-read`.
fn shape(ast: &Ast, e: ExprId) -> &'static str {
    let mut stack = vec![e];
    let mut seen = FxHashSet::default();
    let mut has_sizeof = false;
    while let Some(x) = stack.pop() {
        if !seen.insert(x.index()) || stack.len() > 256 {
            return "deep";
        }
        let n = ast.exprs.get(x);
        let ty = n.ty();
        let scalar_int = ast.types.is_integral(ty) || ast.types.as_enum(ty).is_some();
        match n.kind() {
            ExprKind::IntegerLiteral
            | ExprKind::CharacterLiteral
            | ExprKind::CXXBoolLiteralExpr => {}
            ExprKind::ParenExpr => stack.push(ast.exprs.operand(x)),
            ExprKind::SubstNonTypeTemplateParmExpr => stack.push(ast.exprs.subst_replacement(x)),
            ExprKind::ConstantExpr => {
                if ast.exprs.constant_value(x).is_none() {
                    stack.push(ast.exprs.operand(x));
                }
            }
            ExprKind::DeclRefExpr => {
                let d = ast.exprs.decl(x);
                if ast.decls.enumerator(d).and_then(|v| v.value).is_some() {
                } else if is_var(ast, d) {
                    return "var-read";
                } else {
                    return "declref-other";
                }
            }
            ExprKind::UnaryExprOrTypeTraitExpr => has_sizeof = true,
            k if k.is_cast() && k != ExprKind::CXXReinterpretCastExpr => match ast
                .exprs
                .cast_kind(x)
            {
                CastKind::IntegralCast if ast.types.as_enum(ty).is_some() => return "cast-to-enum",
                CastKind::IntegralCast | CastKind::IntegralToBoolean | CastKind::NoOp => {
                    stack.push(ast.exprs.operand(x));
                }
                CastKind::LValueToRValue => {
                    let op = ast.exprs.operand(x);
                    let o = ast.exprs.get(op);
                    if o.kind() == ExprKind::DeclRefExpr && is_var(ast, ast.exprs.decl(op)) {
                        return "var-read";
                    }
                    return "lvalue-read";
                }
                _ => return "cast-other",
            },
            ExprKind::UnaryOperator => match ast.exprs.unary_op(x) {
                UnaryOp::Plus | UnaryOp::Minus | UnaryOp::Not | UnaryOp::LNot => {
                    stack.push(ast.exprs.operand(x));
                }
                _ => return "unary-other",
            },
            ExprKind::BinaryOperator => {
                let op = ast.exprs.binary_op(x);
                if op == BinaryOp::Comma || op == BinaryOp::Assign {
                    return "binary-other";
                }
                let [a, b] = ast.exprs.operands(x);
                stack.push(b);
                stack.push(a);
            }
            ExprKind::ConditionalOperator => {
                stack.push(ExprId::new(n.a()));
                let [t, f] = ast.exprs.cond_arms(x);
                stack.push(f);
                stack.push(t);
            }
            k => return kind_name(k),
        }
        if !scalar_int {
            return "non-int-type";
        }
    }
    if has_sizeof { "leaf+sizeof" } else { "leaf" }
}

fn is_var(ast: &Ast, d: DeclId) -> bool {
    matches!(ast.decls.get(d).kind(), DeclKind::Var | DeclKind::ParmVar)
}
