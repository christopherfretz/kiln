//! The register-local escape test (spec/oven.md "Register locals",
//! WP-1908): a local the pre-scan keeps in a register allocates no
//! abstract memory, so `EvalRecord::bytes` shows where it lives; the step
//! count never depends on it ([implimits], clang counts statements).

mod common;
use common::*;
use kiln_oven::{Budget, Oven};

/// The bytes and steps of the last evaluation of `src`, which must pass.
fn last(src: &str) -> (u64, u64) {
    let mut tu = Tu::new(src);
    let mut oven = Oven::new(Budget::default());
    if let Err(f) = tu.run(&mut oven) {
        panic!("unexpected failure:\n{}", f.text);
    }
    let r = *oven.stats().last().expect("an evaluation");
    (r.bytes, r.steps)
}

#[track_caller]
fn register(src: &str) {
    assert_eq!(last(src).0, 0, "expected no abstract memory:\n{src}");
}

#[track_caller]
fn memory(src: &str) {
    assert!(last(src).0 > 0, "expected the local in abstract memory:\n{src}");
}

/// [basic.stc.auto]: an initialized scalar whose address is never taken.
#[test]
fn initialized_scalar_is_a_register() {
    register("constexpr int f(int n) { int x = n; x += 1; return x; } static_assert(f(1) == 2);");
}

/// [expr.unary.op]/3: the operand of `&` lives in memory.
#[test]
fn address_taken_is_memory() {
    memory(
        "constexpr int f(int n) { int x = n; int *p = &x; *p += 1; return x; }
         static_assert(f(1) == 2);",
    );
}

/// [stmt.dcl]/3: declared without an initializer and assigned before any
/// other use.
#[test]
fn assigned_first_is_a_register() {
    register(
        "constexpr int f(int n) { int x; int y = n; x = y * 2; x = x + 1; return x; }
         static_assert(f(3) == 7);",
    );
}

/// Assigned on both branches of an `if`, in each iteration of a loop.
#[test]
fn assigned_on_both_branches_is_a_register() {
    register(
        "constexpr int f(int n) {
           int s = 0;
           for (int i = 0; i < n; i = i + 1) {
             int x;
             if (i < 2) x = 1; else { x = 2; }
             s = s + x;
           }
           return s;
         }
         static_assert(f(4) == 6);",
    );
}

/// Assigned on one path only: memory, which diagnoses a read of it.
#[test]
fn assigned_on_one_branch_is_memory() {
    memory(
        "constexpr int f(int b) { int x; if (b) x = 1; x = b ? x : 2; return x; }
         static_assert(f(1) == 1);",
    );
    fails_containing(
        "constexpr int f(int b) { int x; if (b) x = 1; return x; }
         static_assert(f(0) == 0);",
        "read of uninitialized object",
    );
}

/// Assigned only inside a loop body (which may run zero times): memory.
#[test]
fn assigned_in_a_loop_is_memory() {
    memory(
        "constexpr int f(int n) { int x; for (int i = 0; i < 1; i = i + 1) x = n; return x; }
         static_assert(f(5) == 5);",
    );
}

/// [stmt.dcl]/3: a `case` in the variable's scope may be jumped to past its
/// declaration: memory.
#[test]
fn label_in_scope_is_memory() {
    memory(
        "constexpr int f(int n) {
           int r = 0;
           switch (n) { int x; case 0: x = 1; r = x; break; default: r = 2; }
           return r;
         }
         static_assert(f(0) == 1);",
    );
    register(
        "constexpr int f(int n) {
           int r = 0;
           switch (n) { case 0: { int x; x = 1; r = x; break; } default: r = 2; }
           return r;
         }
         static_assert(f(0) == 1);",
    );
}

/// The right operand of the first assignment names the variable: memory,
/// and the read is diagnosed.
#[test]
fn read_in_own_assignment_is_memory() {
    fails_containing(
        "constexpr int f(int n) { int x; x = x + n; return x; }
         static_assert(f(1) == 1);",
        "read of uninitialized object",
    );
}

/// [implimits]: the step count of a register local is clang's (one per
/// statement: the body, two declarations, an assignment, a return).
#[test]
fn steps_do_not_depend_on_the_place() {
    let reg = "constexpr int f(int n) { int x; int y = n; x = y; return x; }
               static_assert(f(3) == 3);";
    let mem = "constexpr int f(int n) { int x; int y = n; x = y; return *&x; }
               static_assert(f(3) == 3);";
    assert_eq!(last(reg), (0, 5));
    let (bytes, steps) = last(mem);
    assert!(bytes > 0);
    assert_eq!(steps, 5);
}
