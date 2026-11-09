// [dcl.constexpr]/6 (C++23, P2448R2): under `-Winvalid-constexpr` the
// never-constant walk (`isPotentialConstantExpr`) follows a loop left by a
// `break`, a `switch`, a try block and the calls before them to "control
// reached end of constexpr function", and reports a `goto`, a static
// variable and the step limit, for a function without parameters too
// (WP-1133).
//@ expect: compile-fail
//@ std: c++23
//@ args: -Winvalid-constexpr -Wno-return-type -Wno-unused-value -ferror-limit=0
//@ strict-diags
//@ diag: 40:15 error "constexpr function never produces a constant expression"
//@ diag: 40:70 note "control reached end of constexpr function"
//@ diag: 41:15 error "constexpr function never produces a constant expression"
//@ diag: 41:49 note "control reached end of constexpr function"
//@ diag: 42:15 error "constexpr function never produces a constant expression"
//@ diag: 42:81 note "control reached end of constexpr function"
//@ diag: 43:15 error "constexpr function never produces a constant expression"
//@ diag: 43:75 note "control reached end of constexpr function"
//@ diag: 44:49 warning "no case matching constant switch condition '2'"
//@ diag: 44:15 error "constexpr function never produces a constant expression"
//@ diag: 44:74 note "control reached end of constexpr function"
//@ diag: 45:15 error "constexpr function never produces a constant expression"
//@ diag: 45:76 note "control reached end of constexpr function"
//@ diag: 46:15 error "constexpr function never produces a constant expression"
//@ diag: 46:72 note "control reached end of constexpr function"
//@ diag: 47:15 error "constexpr function never produces a constant expression"
//@ diag: 47:37 note "subexpression not valid in a constant expression"
//@ diag: 48:15 error "constexpr function never produces a constant expression"
//@ diag: 48:38 note "subexpression not valid in a constant expression"
//@ diag: 49:15 error "constexpr function never produces a constant expression"
//@ diag: 49:54 note "control flows through the definition of a static variable"
//@ diag: 50:15 error "constexpr function never produces a constant expression"
//@ diag: 50:72 note "control flows through the definition of a static variable"
//@ diag: 51:15 error "constexpr function never produces a constant expression"
//@ diag: 51:51 note "constexpr evaluation hit maximum step limit; possible infinite loop?"

constexpr void callee(int) {}

// Reported.
constexpr int break_then_end(int x) { while (true) { x + 1; break; } }
constexpr int call_then_end(int x) { callee(x); }
constexpr int call_in_loop_then_end(int x) { while (true) { callee(x); break; } }
constexpr int local_then_loop(int x) { int y = 0; while (true) { break; } }
constexpr int no_case_then_end(int x) { switch (2) { case 1: return 1; } }
constexpr int break_switch_then_end(int x) { switch (1) { case 1: break; } }
constexpr int try_then_end(int x) { try { callee(x); } catch (...) { } }
constexpr int goto_forward(int x) { goto l; l: return 0; }
constexpr int no_parameters_goto() { goto l; l: return 0; }
constexpr int no_parameters_try() { try { static int i = 0; return i; } catch (...) { return 0; } }
constexpr int no_parameters_switch() { switch (1) { case 1: static int i = 0; } return 0; }
constexpr int no_parameters_loop() { while (true) { } }

// Silent: an unknown condition stops the walk; after a side effect, leaving
// a block with a local fails quietly.
constexpr int call_local_then_loop(int x) { callee(x); int y = 0; while (true) { break; } }
constexpr int unknown_if_then_end(int x) { if (x) return 1; while (true) { break; } }
constexpr int local_loop_then_end(int x) { while (true) { x + 1; int y = 0; break; } }
