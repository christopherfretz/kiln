// [dcl.constexpr]/6 (C++20): the never-constant walk
// (`isPotentialConstantExpr`) runs a `switch` with a constant condition: it
// finds the case label like the evaluator does (searching into nested
// blocks, `if` and loop bodies) and reports a static variable reached from
// there (WP-1133).
//@ expect: compile-fail
//@ std: c++20
//@ args: -Wno-unused-value -Wno-c++23-extensions -ferror-limit=0
//@ strict-diags
//@ diag: 51:15 error "constexpr function never produces a constant expression"
//@ diag: 51:79 note "control flows through the definition of a static variable"
//@ diag: 52:40 warning "no case matching constant switch condition '2'"
//@ diag: 52:15 error "constexpr function never produces a constant expression"
//@ diag: 52:84 note "control flows through the definition of a static variable"
//@ diag: 53:15 error "constexpr function never produces a constant expression"
//@ diag: 53:101 note "control flows through the definition of a static variable"
//@ diag: 54:15 error "constexpr function never produces a constant expression"
//@ diag: 54:112 note "control flows through the definition of a static variable"
//@ diag: 55:15 error "constexpr function never produces a constant expression"
//@ diag: 55:71 note "control flows through the definition of a static variable"
//@ diag: 56:15 error "constexpr function never produces a constant expression"
//@ diag: 56:80 note "control flows through the definition of a static variable"
//@ diag: 57:15 error "constexpr function never produces a constant expression"
//@ diag: 57:79 note "control flows through the definition of a static variable"
//@ diag: 58:15 error "constexpr function never produces a constant expression"
//@ diag: 58:85 note "control flows through the definition of a static variable"
//@ diag: 59:15 error "constexpr function never produces a constant expression"
//@ diag: 59:72 note "control flows through the definition of a static variable"
//@ diag: 60:38 warning "no case matching constant switch condition '1'"
//@ diag: 60:15 error "constexpr function never produces a constant expression"
//@ diag: 60:56 note "control flows through the definition of a static variable"
//@ diag: 61:15 error "constexpr function never produces a constant expression"
//@ diag: 61:63 note "control flows through the definition of a static variable"
//@ diag: 62:15 error "constexpr function never produces a constant expression"
//@ diag: 62:73 note "control flows through the definition of a static variable"
//@ diag: 63:15 error "constexpr function never produces a constant expression"
//@ diag: 63:110 note "control flows through the definition of a static variable"
//@ diag: 64:15 error "constexpr function never produces a constant expression"
//@ diag: 64:92 note "control flows through the definition of a static variable"
//@ diag: 65:15 error "constexpr function never produces a constant expression"
//@ diag: 65:63 note "control flows through the definition of a static variable"
//@ diag: 66:15 error "constexpr function never produces a constant expression"
//@ diag: 66:76 note "control flows through the definition of a static variable"

constexpr int g(int);

// Reported: the walk jumps to the matching case (or default) wherever it is
// nested, falls through, and leaves at a break. Statements before the label
// are skipped, so their side effects are never noted, but a static variable
// definition skipped on the way still fails.
constexpr int fall_through(int x) { switch (1) { case 1: ; case 2: static int i = 0; } return 0; }
constexpr int no_case(int x) { switch (2) { case 1: static int i = 0; } static int j = 0; return j; }
constexpr int break_out(int x) { switch (1) { case 1: break; case 2: static int i = 0; } static int j = 0; return j; }
constexpr int break_switch_then_loop(int x) { while (true) { switch (1) { case 1: break; } break; } static int j = 0; return j; }
constexpr int default_label(int x) { switch (3) { default: static int i = 0; case 1: ; } return 0; }
constexpr int case_in_block(int x) { switch (1) { case 0: { case 1: static int i = 0; } } return 0; }
constexpr int case_in_if(int x) { switch (1) { if (true) { case 1: static int i = 0; } } return 0; }
constexpr int case_in_while(int x) { switch (1) { while (true) { case 1: static int i = 0; } } return 0; }
constexpr int case_in_do(int x) { switch (1) { do { case 1: static int i = 0; } while (0); } return 0; }
constexpr int empty(int x) { switch (1) { } static int i = 0; return i; }
constexpr int unbraced(int x) { switch (1) case 1: static int i = 0; return 0; }
constexpr int labelled_case(int x) { switch (1) { l: case 1: static int i = 0; } return 0; }
constexpr int skipped_side_effect(int x) { switch (1) { case 0: x + 1; case 1: ; } { int y = 0; } static int i = 0; return i; }
constexpr int skipped_declaration(int x) { switch (1) { int y; case 1: break; } static int i = 0; return i; }
constexpr int skipped_static(int x) { switch (1) { static int i = 0; case 1: break; } return 0; }
constexpr int skipped_case_static(int x) { switch (1) { case 0: static int i = 0; case 1: break; } return 0; }

// Silent: an unknown condition stops the walk; after a side effect, leaving
// a block with a local (the init-statement's, a case's, a later block's)
// fails quietly.
constexpr int unknown_condition(int x) { switch (x) { case 1: static int i = 0; } return 0; }
constexpr int init_statement(int x) { x + 1; switch (int k = 1; k) { case 1: break; } static int i = 0; return i; }
constexpr int local_in_case(int x) { x + 1; switch (1) { case 1: int y = 0; break; } static int i = 0; return i; }
constexpr int block_after_switch(int x) { switch (1) { case 1: x + 1; break; } { int y = 0; } static int i = 0; return i; }
constexpr int unknown_case_body(int x) { switch (1) { case 1: g(x); } { int y = 0; } static int i = 0; return i; }
