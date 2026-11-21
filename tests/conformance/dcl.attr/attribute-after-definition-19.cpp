// [clang ext] clang 19 behaviour (kiln -fclang-compat=19) of
// "attribute declaration must precede definition": `[[deprecated]]`,
// `[[nodiscard]]` and `[[maybe_unused]]` added after the definition warn
// as every other attribute does; clang 22 accepts them in the standard
// syntax (see attribute-after-definition.cpp). spec/sema.md §4.5, WP-1807.
//@ expect: compile-ok
//@ requires: clang<22
//@ strict-diags
//@ diag: 20:3 warning "attribute declaration must precede definition"
//@ diag: 19:5 note "previous definition is here"
//@ diag: 22:3 warning "attribute declaration must precede definition"
//@ diag: 21:5 note "previous definition is here"
//@ diag: 24:3 warning "attribute declaration must precede definition"
//@ diag: 23:5 note "previous definition is here"
//@ diag: 26:3 warning "attribute declaration must precede definition"
//@ diag: 25:5 note "previous definition is here"
//@ diag: 28:10 warning "attribute declaration must precede definition"
//@ diag: 27:8 note "previous definition is here"
int k2() { return 0; }
[[nodiscard]] int k2();
int k3() { return 0; }
[[deprecated("use k4")]] int k3();
int k4() { return 0; }
[[maybe_unused]] int k4();
int k5() { return 0; }
[[gnu::deprecated]] int k5();
struct S {};
struct [[deprecated]] S;
