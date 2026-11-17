// [temp.param]/4, [temp.arg.general]/1, [temp.variadic]: a type-constraint
// C<A...> names the concept-id C<X, A...> with the constrained type first,
// and its list is checked as written: a pack expansion that lands on a
// non-pack parameter of the concept, too many or too few arguments, or no
// list for a concept that needs more than one argument. Checked where the
// constraint is formed: a constrained template parameter, a function or
// lambda parameter's placeholder, a return-type-requirement (clang 22 and
// GCC 16.1 reject each). Found running GCC's C++ testsuite. WP-1610.
//@ std: c++20
//@ expect: compile-fail
//@ strict-diags
//@ diag: 38:15 error "pack expansion used as argument for non-pack parameter of concept"
//@ diag: 34:26 note "template parameter is declared here"
//@ diag: 39:15 error "pack expansion used as argument for non-pack parameter of concept"
//@ diag: 35:26 note "template parameter is declared here"
//@ diag: 40:15 error "pack expansion used as argument for non-pack parameter of concept"
//@ diag: 34:26 note "template parameter is declared here"
//@ diag: 41:13 error "pack expansion used as argument for non-pack parameter of concept"
//@ diag: 34:26 note "template parameter is declared here"
//@ diag: 42:13 error "pack expansion used as argument for non-pack parameter of concept"
//@ diag: 34:26 note "template parameter is declared here"
//@ diag: 43:27 error "pack expansion used as argument for non-pack parameter of concept"
//@ diag: 34:26 note "template parameter is declared here"
//@ diag: 44:75 error "pack expansion used as argument for non-pack parameter of concept"
//@ diag: 34:26 note "template parameter is declared here"
//@ diag: 46:11 error "too many template arguments for concept 'G'"
//@ diag: 34:37 note "template is declared here"
//@ diag: 47:11 error "too few template arguments for concept 'G'"
//@ diag: 34:37 note "template is declared here"
//@ diag: 48:11 error "too many template arguments for concept 'One'"
//@ diag: 36:28 note "template is declared here"
//@ diag: 49:11 error "'G' requires more than 1 template argument; provide the remaining arguments explicitly to use it here"
//@ diag: 50:9 error "'G' requires more than 1 template argument; provide the remaining arguments explicitly to use it here"
template <class T, class U> concept G = true;
template <class T, class U, class... Vs> concept H = true;
template <class T> concept One = true;
template <class... Us> struct S {
  template <G<Us...> X> void f1(X);
  template <H<Us...> X> void f2(X);
  template <G<Us..., int> X> void f3(X);
  void f4(G<Us...> auto x);
  void f5(G<Us...> auto... x);
  using L = decltype([](G<Us...> auto) {});
  template <class X> static constexpr bool c = requires(X x) { { x } -> G<Us...>; };
};
template <G<int, long> X> void g1();
template <G<> X> void g2();
template <One<int> X> void g3();
template <G X> void g4();
void g5(G auto x);
