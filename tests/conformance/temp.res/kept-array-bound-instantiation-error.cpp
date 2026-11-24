// [temp.res.general]/6, [temp.inst]/11: a local array whose bound's value
// is known at the definition but whose expression is
// instantiation-dependent (`sizeof(sizeof(f(t)))`) keeps the expression,
// which is substituted when the function template is instantiated: an
// ill-formed bound is an error in that specialization (here `f(X)` has
// no viable function), not silently accepted with the value. The same
// holds for a member template's bound when its class is instantiated.
// Reduced from real code.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 23:61 error "no matching function for call to 'f'"
//@ diag: 25:9 note "in instantiation of template class 'A<X>' requested here"
//@ diag: 18:15 note "candidate function not viable: no known conversion from 'X' to 'int' for 1st argument"
//@ diag: 19:54 error "no matching function for call to 'f'"
//@ diag: 20:9 note "in instantiation of function template specialization 'g<X>' requested here"
//@ diag: 18:15 note "candidate function not viable: no known conversion from 'X' to 'int' for 1st argument"
struct X {};
constexpr int f(int) { return 0; }
template <class T> int g(T t) { char a[sizeof(sizeof(f(t)))]; return sizeof a; }
int u = g(X());
int v = g(1);
template <class T> struct A {
  template <class U> static int h(U, char (*)[sizeof(sizeof(f(T()) + f(U())))]);
};
int w = sizeof(A<X>);
