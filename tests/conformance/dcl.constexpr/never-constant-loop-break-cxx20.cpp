// [dcl.constexpr]/6 (C++20): the never-constant walk
// (`isPotentialConstantExpr`) runs loops whose conditions are constant: a
// `while (true)`, `for (;;)` or `do ... while (true)` body is walked, and a
// `break` (or a false condition) leaves the loop, so a static variable after
// it is reported. `continue` goes to the condition; an increment that cannot
// be evaluated only notes a side effect (WP-1133).
//@ expect: compile-fail
//@ std: c++20
//@ args: -Wno-unused-value -Wno-c++23-extensions -ferror-limit=0
//@ strict-diags
//@ diag: 46:15 error "constexpr function never produces a constant expression"
//@ diag: 46:71 note "control flows through the definition of a static variable"
//@ diag: 47:15 error "constexpr function never produces a constant expression"
//@ diag: 47:65 note "control flows through the definition of a static variable"
//@ diag: 48:15 error "constexpr function never produces a constant expression"
//@ diag: 48:66 note "control flows through the definition of a static variable"
//@ diag: 49:15 error "constexpr function never produces a constant expression"
//@ diag: 49:72 note "control flows through the definition of a static variable"
//@ diag: 50:15 error "constexpr function never produces a constant expression"
//@ diag: 50:65 note "control flows through the definition of a static variable"
//@ diag: 51:15 error "constexpr function never produces a constant expression"
//@ diag: 51:79 note "control flows through the definition of a static variable"
//@ diag: 52:15 error "constexpr function never produces a constant expression"
//@ diag: 52:62 note "control flows through the definition of a static variable"
//@ diag: 53:15 error "constexpr function never produces a constant expression"
//@ diag: 53:70 note "control flows through the definition of a static variable"
//@ diag: 54:15 error "constexpr function never produces a constant expression"
//@ diag: 54:93 note "control flows through the definition of a static variable"
//@ diag: 55:15 error "constexpr function never produces a constant expression"
//@ diag: 55:92 note "control flows through the definition of a static variable"
//@ diag: 56:15 error "constexpr function never produces a constant expression"
//@ diag: 56:95 note "control flows through the definition of a static variable"
//@ diag: 57:15 error "constexpr function never produces a constant expression"
//@ diag: 57:99 note "control flows through the definition of a static variable"
//@ diag: 58:15 error "constexpr function never produces a constant expression"
//@ diag: 58:75 note "control flows through the definition of a static variable"
//@ diag: 59:15 error "constexpr function never produces a constant expression"
//@ diag: 59:79 note "control flows through the definition of a static variable"
//@ diag: 60:15 error "constexpr function never produces a constant expression"
//@ diag: 60:90 note "control flows through the definition of a static variable"

constexpr void callee(int) {}

// Reported: the walk follows the loop to the break (or out of a false
// condition) and on to the static variable.
constexpr int while_break(int x) { while (true) { break; } static int i = 0; return i; }
constexpr int for_break(int x) { for (;;) { break; } static int i = 0; return i; }
constexpr int do_false(int x) { do { } while (false); static int i = 0; return i; }
constexpr int do_break(int x) { do { break; } while (true); static int i = 0; return i; }
constexpr int while_false(int x) { while (false) { } static int i = 0; return i; }
constexpr int for_init_break(int x) { for (int j = 0;;) { break; } static int i = 0; return i; }
constexpr int body_static(int x) { while (true) { static int i = 0; } return 0; }
constexpr int unbraced_break(int x) { while (true) break; static int i = 0; return i; }
constexpr int if_break_continue(int x) { for (;;) { if (true) break; continue; } static int i = 0; return i; }
constexpr int nested_break(int x) { for (;;) { while (true) { break; } break; } static int i = 0; return i; }
constexpr int increment_break(int x) { for (int k = 0; ; ++k) { if (true) break; } static int i = 0; return i; }
constexpr int unknown_increment_break(int x) { for (;; callee(x)) { if (true) break; } static int i = 0; return i; }
constexpr int unknown_increment_static(int x) { for (;; x++) { static int i = 0; } return 0; }
constexpr int do_side_effect(int x) { do { x + 1; } while (false); static int i = 0; return i; }
constexpr int do_continue(int x) { do { int y = 0; continue; } while (false); static int i = 0; return i; }

// Silent: an unknown condition stops the walk; after a side effect, leaving
// a block with a local (the loop body, a for-init-statement, a later block)
// fails quietly.
constexpr int body_local_break(int x) { while (true) { int y = 0; x + 1; break; } static int i = 0; return i; }
constexpr int unknown_break(int x) { while (true) { if (x) break; } static int i = 0; return i; }
constexpr int init_after_side_effect(int x) { x + 1; for (int j = 0;;) { break; } static int i = 0; return i; }
constexpr int block_after_loop(int x) { while (true) { x + 1; break; } { int y = 0; } static int i = 0; return i; }
constexpr int local_after_side_effect(int x) { while (true) { x + 1; int y = 0; break; } static int i = 0; return i; }
