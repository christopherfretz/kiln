// [dcl.constexpr]/6 (C++20): the never-constant walk
// (`isPotentialConstantExpr`) runs an infinite loop like the evaluator does:
// each statement it evaluates takes a step, and when the walk runs out of
// steps (here `-fconstexpr-steps=1001`) it reports "constexpr
// evaluation hit maximum step limit" at the statement it was about to
// evaluate. A loop the walk leaves quietly (after a side effect, a block
// holding a local cannot be destroyed) is not reported (WP-1133).
//@ expect: compile-fail
//@ std: c++20
//@ args: -Wno-unused-value -Wno-c++23-extensions -Wno-empty-body -ferror-limit=0 -fconstexpr-steps=1001
//@ strict-diags
//@ diag: 57:15 error "constexpr function never produces a constant expression"
//@ diag: 57:65 note "control flows through the definition of a static variable"
//@ diag: 58:15 error "constexpr function never produces a constant expression"
//@ diag: 58:43 note "constexpr evaluation hit maximum step limit; possible infinite loop?"
//@ diag: 59:15 error "constexpr function never produces a constant expression"
//@ diag: 59:56 note "constexpr evaluation hit maximum step limit; possible infinite loop?"
//@ diag: 60:15 error "constexpr function never produces a constant expression"
//@ diag: 60:55 note "constexpr evaluation hit maximum step limit; possible infinite loop?"
//@ diag: 61:15 error "constexpr function never produces a constant expression"
//@ diag: 61:49 note "constexpr evaluation hit maximum step limit; possible infinite loop?"
//@ diag: 62:15 error "constexpr function never produces a constant expression"
//@ diag: 62:90 note "control flows through the definition of a static variable"
//@ diag: 63:15 error "constexpr function never produces a constant expression"
//@ diag: 63:47 note "constexpr evaluation hit maximum step limit; possible infinite loop?"
//@ diag: 64:15 error "constexpr function never produces a constant expression"
//@ diag: 64:35 note "constexpr evaluation hit maximum step limit; possible infinite loop?"
//@ diag: 65:15 error "constexpr function never produces a constant expression"
//@ diag: 65:44 note "constexpr evaluation hit maximum step limit; possible infinite loop?"
//@ diag: 66:15 error "constexpr function never produces a constant expression"
//@ diag: 66:76 note "constexpr evaluation hit maximum step limit; possible infinite loop?"
//@ diag: 67:15 error "constexpr function never produces a constant expression"
//@ diag: 67:58 note "constexpr evaluation hit maximum step limit; possible infinite loop?"
//@ diag: 68:15 error "constexpr function never produces a constant expression"
//@ diag: 68:59 note "constexpr evaluation hit maximum step limit; possible infinite loop?"
//@ diag: 69:15 error "constexpr function never produces a constant expression"
//@ diag: 69:43 note "constexpr evaluation hit maximum step limit; possible infinite loop?"
//@ diag: 70:15 error "constexpr function never produces a constant expression"
//@ diag: 70:50 note "constexpr evaluation hit maximum step limit; possible infinite loop?"
//@ diag: 71:15 error "constexpr function never produces a constant expression"
//@ diag: 71:74 note "constexpr evaluation hit maximum step limit; possible infinite loop?"
//@ diag: 72:15 error "constexpr function never produces a constant expression"
//@ diag: 72:78 note "constexpr evaluation hit maximum step limit; possible infinite loop?"
//@ diag: 73:15 error "constexpr function never produces a constant expression"
//@ diag: 73:58 note "constexpr evaluation hit maximum step limit; possible infinite loop?"
//@ diag: 74:15 error "constexpr function never produces a constant expression"
//@ diag: 74:53 note "constexpr evaluation hit maximum step limit; possible infinite loop?"
//@ diag: 75:15 error "constexpr function never produces a constant expression"
//@ diag: 75:46 note "constexpr evaluation hit maximum step limit; possible infinite loop?"
//@ diag: 76:16 error "constexpr function never produces a constant expression"
//@ diag: 76:48 note "constexpr evaluation hit maximum step limit; possible infinite loop?"

constexpr void callee(int) {}
struct S { int v; };

// Reported: a static variable in the body, or the step limit.
constexpr int static_in_body(int x) { while (true) { static int i = 0; } return 0; }
constexpr int empty_for(int x) { for (;;) { } return 0; }
constexpr int side_effect_body(int x) { while (true) { x + 1; } return 0; }
constexpr int unknown_increment(int x) { for (;; x++) { } return 0; }
constexpr int call_body(int x) { while (true) { callee(x); } return 0; }
constexpr int block_static_side_effect(int x) { while (true) { { int y = 0; } static int i = 0; x + 1; } return 0; }
constexpr int continue_body(int x) { for (;;) { if (true) continue; } return 0; }
constexpr int do_true(int x) { do { } while (true); return 0; }
constexpr int do_side_effect(int x) { do { x + 1; } while (true); return 0; }
constexpr int switch_continue(int x) { while (true) { switch (1) { case 1: continue; } } return 0; }
constexpr int for_init_block(int x) { for (int j = 0;; ) { { } } return 0; }
constexpr int attributed(int x) { while (true) [[likely]] { } return 0; }
constexpr int null_body(int x) { for (;;) ; return 0; }
constexpr int local_body(int x) { while (true) { int y = 0; } return 0; }
constexpr int nested_infinite(int x) { switch (1) { case 1: while (true) { } } return 0; }
constexpr int else_side_effect(int x) { while (true) { if (false) { } else { x + 1; } } return 0; }
constexpr int call_increment(int x) { for (;; callee(x)) { } return 0; }
constexpr int temp_increment(int x) { for (;; S{1}) { } return 0; }
constexpr int no_parameters() { while (true) { } return 0; }
constexpr void void_no_parameters() { for (;;) { } }

// Silent: after a side effect, leaving a block with a local fails quietly.
constexpr int block_then_side_effect(int x) { while (true) { { int y = 0; } x + 1; } return 0; }
constexpr int side_effect_block_static(int x) { while (true) { x + 1; { int y = 0; } static int i = 0; } return 0; }
constexpr int side_effect_local(int x) { while (true) { x + 1; int y = 0; } return 0; }
constexpr int side_effect_temp_increment(int x) { x + 1; for (;; S{1}) { } return 0; }
