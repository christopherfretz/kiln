// [dcl.constexpr]/6 (C++20): clang's never-constant walk searches a
// `switch` body for the selected label like the evaluator does; it does
// not enter a try-block or a statement expression, so a label found only
// there gives "this use of statement expressions is not supported" at the
// label. With no matching label, the body is skipped (WP-1150).
//@ expect: compile-fail
//@ std: c++20
//@ args: -Wno-unused-value -Wno-c++23-extensions -Wno-gnu-statement-expression -Wno-uninitialized -ferror-limit=0
//@ strict-diags
//@ diag: 35:50 error "cannot jump from switch statement to this case label"
//@ diag: 35:44 note "jump bypasses initialization of try block"
//@ diag: 35:15 error "constexpr function never produces a constant expression"
//@ diag: 35:50 note "this use of statement expressions is not supported in a constant expression"
//@ diag: 36:64 error "cannot jump from switch statement to this case label"
//@ diag: 36:61 note "jump enters a statement expression"
//@ diag: 36:15 error "constexpr function never produces a constant expression"
//@ diag: 36:64 note "this use of statement expressions is not supported in a constant expression"
//@ diag: 37:73 error "cannot jump from switch statement to this case label"
//@ diag: 37:67 note "jump bypasses initialization of try block"
//@ diag: 37:15 error "constexpr function never produces a constant expression"
//@ diag: 37:73 note "this use of statement expressions is not supported in a constant expression"
//@ diag: 41:41 warning "no case matching constant switch condition '2'"
//@ diag: 41:52 error "cannot jump from switch statement to this case label"
//@ diag: 41:46 note "jump bypasses initialization of try block"
//@ diag: 41:15 error "constexpr function never produces a constant expression"
//@ diag: 41:97 note "control flows through the definition of a static variable"
//@ diag: 42:41 warning "no case matching constant switch condition '2'"
//@ diag: 42:15 error "constexpr function never produces a constant expression"
//@ diag: 42:78 note "control flows through the definition of a static variable"
//@ diag: 43:15 error "constexpr function never produces a constant expression"
//@ diag: 43:85 note "control flows through the definition of a static variable"

// Diagnosed: clang's case search does not enter a try-block or a
// statement expression, so the selected label is not found.
constexpr int in_try(int x) { switch (1) { try { case 1: break; } catch (...) {} } return 0; }
constexpr int in_statement_expression(int x) { switch (1) { ({ case 1: 0; }); } return 0; }
constexpr int default_in_try(int x) { switch (5) { case 1: break; try { default: break; } catch (...) {} } return 0; }

// Diagnosed: no label matches, so the body is skipped; a label in a
// block or an `if` branch is found.
constexpr int no_match(int x) { switch (2) { try { case 1: break; } catch (...) {} } static int i = 0; return i; }
constexpr int in_block(int x) { switch (2) { { case 1: break; } } static int i = 0; return i; }
constexpr int in_if(int x) { switch (1) { if (true) { case 1: break; } } static int i = 0; return i; }

// Not diagnosed: the label is found and the function returns.
constexpr int found(int x) { switch (1) { { case 1: return 0; } } return 1; }
