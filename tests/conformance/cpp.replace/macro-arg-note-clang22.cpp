// [cpp.replace], [cpp.subst]: a diagnostic on a token that came from a
// macro argument gets an "expanded from macro" note for the macro whose
// argument it was, unless the diagnostic carries no source range.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 31:7 warning "equality comparison result unused"
//@ diag: 24:14 note "expanded from macro 'M'"
//@ diag: 31:7 note "use '=' to turn this equality comparison into an assignment"
//@ diag: 24:14 note "expanded from macro 'M'"
//@ diag: 34:5 warning "ignoring return value of function declared with 'nodiscard' attribute"
//@ diag: 24:14 note "expanded from macro 'M'"
//@ diag: 35:6 error "static assertion failed due to requirement 'sizeof(int) == 2': m"
//@ diag: 26:29 note "expanded from macro 'SA'"
//@ diag: 35:18 note "expression evaluates to '4 == 2'"
//@ diag: 26:29 note "expanded from macro 'SA'"
//@ diag: 36:5 error "no matching function for call to 'g'"
//@ diag: 28:6 note "candidate function not viable: no known conversion from 'S' to 'int' for 1st argument"
//@ diag: 37:7 error "invalid operands to binary expression ('S' and 'int')"
//@ diag: 32:5 warning "expression result unused"
//@ diag: 24:14 note "expanded from macro 'M'"
//@ diag: 33:7 warning "expression result unused"
//@ requires: clang>=22
// clang 22 spelling of macro-arg-note.cpp (WP-394).
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
