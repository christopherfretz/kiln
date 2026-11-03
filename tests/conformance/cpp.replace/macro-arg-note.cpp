// [cpp.replace], [cpp.subst]: a diagnostic on a token that came from a
// macro argument gets an "expanded from macro" note for the macro whose
// argument it was, unless the diagnostic carries no source range.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 35:7 warning "equality comparison result unused"
//@ diag: 28:14 note "expanded from macro 'M'"
//@ diag: 35:7 note "use '=' to turn this equality comparison into an assignment"
//@ diag: 28:14 note "expanded from macro 'M'"
//@ diag: 36:5 warning "expression result unused"
//@ diag: 28:14 note "expanded from macro 'M'"
//@ diag: 37:7 warning "expression result unused"
//@ diag: 29:16 note "expanded from macro 'P'"
//@ diag: 28:14 note "expanded from macro 'M'"
//@ diag: 38:5 warning "ignoring return value of function declared with 'nodiscard' attribute"
//@ diag: 28:14 note "expanded from macro 'M'"
//@ diag: 39:6 error "static assertion failed due to requirement 'sizeof(int) == 2': m"
//@ diag: 30:29 note "expanded from macro 'SA'"
//@ diag: 39:18 note "expression evaluates to '4 == 2'"
//@ diag: 30:29 note "expanded from macro 'SA'"
//@ diag: 40:5 error "no matching function for call to 'g'"
//@ diag: 28:14 note "expanded from macro 'M'"
//@ diag: 32:6 note "candidate function not viable: no known conversion from 'S' to 'int' for 1st argument"
//@ diag: 41:7 error "invalid operands to binary expression ('S' and 'int')"
//@ diag: 28:14 note "expanded from macro 'M'"
//@ requires: clang<22
// clang 19 spelling; macro-arg-note-clang22.cpp has clang 22's (WP-394).
#define M(x) x
#define P(x) M(x)
#define SA(c) static_assert(c, "m")
struct S {};
void g(int);
[[nodiscard]] int nd();
void f(int a, S s) {
  M(a == 1);
  M(a);
  P(a + 1);
  M(nd());
  SA(sizeof(int) == 2);
  M(g(s));
  M(s + 1);
}
