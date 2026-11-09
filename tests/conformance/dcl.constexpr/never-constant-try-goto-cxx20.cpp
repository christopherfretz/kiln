// [dcl.constexpr]/6 (C++20): the never-constant walk
// (`isPotentialConstantExpr`) walks a try block (never a handler; a
// function-try-block's body likewise) and goes on after it. A `goto` it
// reaches, forward or backward, fails with "subexpression not valid in a
// constant expression"; a label is just its sub-statement (WP-1133).
//@ expect: compile-fail
//@ std: c++20
//@ args: -Wno-unused-value -Wno-c++23-extensions -ferror-limit=0
//@ strict-diags
//@ diag: 33:15 error "constexpr function never produces a constant expression"
//@ diag: 33:55 note "control flows through the definition of a static variable"
//@ diag: 34:15 error "constexpr function never produces a constant expression"
//@ diag: 34:95 note "control flows through the definition of a static variable"
//@ diag: 35:15 error "constexpr function never produces a constant expression"
//@ diag: 35:85 note "control flows through the definition of a static variable"
//@ diag: 36:15 error "constexpr function never produces a constant expression"
//@ diag: 36:43 note "subexpression not valid in a constant expression"
//@ diag: 37:15 error "constexpr function never produces a constant expression"
//@ diag: 37:37 note "subexpression not valid in a constant expression"
//@ diag: 38:15 error "constexpr function never produces a constant expression"
//@ diag: 38:44 note "control flows through the definition of a static variable"
//@ diag: 39:15 error "constexpr function never produces a constant expression"
//@ diag: 39:54 note "subexpression not valid in a constant expression"
//@ diag: 40:15 error "constexpr function never produces a constant expression"
//@ diag: 40:45 note "subexpression not valid in a constant expression"
//@ diag: 41:15 error "constexpr function never produces a constant expression"
//@ diag: 41:58 note "control flows through the definition of a static variable"

constexpr void callee(int) {}

// Reported: a static variable reached in the try block or after it; a
// throw or goto reached in the walk.
constexpr int static_in_try(int x) { try { static int i = 0; return i; } catch (...) { return 0; } }
constexpr int static_in_handler(int x) { try { } catch (...) { static int i = 0; } static int j = 0; return j; }
constexpr int side_effect_in_try(int x) { try { x + 1; } catch (...) { } static int i = 0; return i; }
constexpr int throw_in_try(int x) { try { throw 1; } catch (...) { } static int i = 0; return i; }
constexpr int forward_goto(int x) { goto l; l: return 0; }
constexpr int label(int x) { l: static int i = 0; return i; }
constexpr int goto_after_side_effect(int x) { x + 1; goto l; l: return 0; }
constexpr int goto_in_if(int x) { if (true) goto l; return 1; l: return 0; }
constexpr int function_try_block(int x) try { static int i = 0; return i; } catch (...) { return 0; }

// Silent: an unknown condition stops the walk before the goto; after a side
// effect, leaving a block with a local fails quietly; the handler is never
// walked.
constexpr int backward_goto(int x) { l: if (x) goto l; return 0; }
constexpr int local_in_try(int x) { try { x + 1; int y = 0; } catch (...) { } static int i = 0; return i; }
constexpr int return_in_try(int x) { try { return x; } catch (...) { static int i = 0; return i; } }
constexpr int unknown_goto(int x) { if (x) goto l; static int i = 0; l: return 0; }
