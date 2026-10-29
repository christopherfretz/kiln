// [over.match.call], [basic.lookup.argdep]/3: a call whose callee names
// one function without argument-dependent lookup (qualified,
// parenthesized, a member) reports argument errors directly, where an
// unqualified name is resolved as an overload set of one ("no matching
// function").
//@ expect: compile-fail
//@ strict-diags
//@ diag: 35:8 error "cannot initialize a parameter of type 'int *' with an rvalue of type 'int'"
//@ diag: 25:13 note "passing argument to parameter 'p' here"
//@ diag: 36:7 error "cannot initialize a parameter of type 'int *' with an rvalue of type 'int'"
//@ diag: 28:13 note "passing argument to parameter 'p' here"
//@ diag: 37:3 error "no matching function for call to 'g'"
//@ diag: 28:6 note "candidate function not viable: no known conversion from 'int' to 'int *' for 1st argument"
//@ diag: 38:9 error "too few arguments to function call, expected 2, have 1"
//@ diag: 26:6 note "'h' declared here"
//@ diag: 39:3 error "no matching function for call to 'k'"
//@ diag: 29:6 note "candidate function not viable: requires 2 arguments, but 1 was provided"
//@ diag: 40:13 error "too many arguments to function call, expected 2, have 3"
//@ diag: 29:6 note "'k' declared here"
//@ diag: 41:7 error "cannot initialize a parameter of type 'int *' with an rvalue of type 'int'"
//@ diag: 31:15 note "passing argument to parameter 'p' here"
//@ diag: 42:9 error "cannot initialize a parameter of type 'int *' with an rvalue of type 'int'"
//@ diag: 32:23 note "passing argument to parameter 'p' here"
namespace N {
void f(int *p);
void h(int a, int b);
} // namespace N
void g(int *p);
void k(int a, int b);
struct S {
  void m(int *p);
  static void sm(int *p);
};
void t(S s) {
  N::f(1);
  (g)(1);
  g(1);
  N::h(1);
  k(1);
  (k)(1, 2, 3);
  s.m(1);
  S::sm(1);
}
