// [temp.param]/17, [temp.arg.general]/1, [temp.deduct.call]: a member
// template whose non-type parameter is a pack expansion over the
// enclosing `Ts` takes exactly one argument per element of `Ts` once it is
// substituted; too few or too many is an error, and for a function
// template too few explicit arguments with nothing to deduce the rest
// from is clang's "deduced too few arguments for expanded pack".
//@ expect: compile-fail
//@ std: c++20
//@ strict-diags
//@ diag: 24:15 error "too few template arguments for class template"
//@ diag: 21:29 note "template is declared here"
//@ diag: 25:15 error "too many template arguments for class template"
//@ diag: 21:29 note "template is declared here"
//@ diag: 26:6 error "too many template arguments for class template"
//@ diag: 21:29 note "template is declared here"
//@ diag: 28:3 error "no matching function for call to 'f'"
//@ diag: 22:34 note "candidate template ignored: deduced too few arguments for expanded pack 'a'; no argument for 2nd expanded parameter in deduced argument pack <1>"
//@ diag: 29:3 error "no matching function for call to 'f'"
//@ diag: 22:34 note "candidate template ignored: substitution failure: too many template arguments for function template 'f'"
template <class... Ts> struct O {
  template <Ts... a> struct A {};
  template <Ts... a> static void f() {}
};
O<int, char>::A<1> x1;
O<int, char>::A<1, 'x', 2> x2;
O<>::A<1> x4;
void g() {
  O<int, char>::f<1>();
  O<int, char>::f<1, 2, 3>();
}
