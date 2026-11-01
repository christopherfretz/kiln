// [expr.prim.lambda.closure]/2, Itanium ABI 5.1.8 (WP-203): a closure type
// created in an instantiated default argument is a local class of the
// specialization (clang `SubstDefaultArgument` runs in the function's
// context). In a function template it has no mangling number and is the
// internal `$_N` of that function (`_ZZ4dfltIiEiPFivEEN3$_0...`); in a
// member function defined in its class it is numbered per parameter
// (`d_`). An undeduced `auto` / `decltype(auto)` is not a substitution
// candidate (clang `isTypeSubstitutable`), so a nested generic lambda's
// return type stays `Da`.
template <class T> int dflt(int (*f)() = [] { return 1; }) { return f(); }
template <class T>
int two(int a, int (*f)() = [] { return 1; }, int (*g)() = [] { return 2; }) {
  return a + f() + g();
}
struct S {
  template <class T> int m(int (*f)() = [] { return 2; }) { return f(); }
};
template <class T> struct C {
  int m(int (*f)() = [] { return 3; }) { return f(); }
};
int nt(int (*f)() = [] { return 4; }) { return f(); }
template <class T> auto outer(T t) {
  auto g = [](auto x) { return [x](auto y) { return x + y; }; };
  return g(t)(t);
}
template <class T> decltype(auto) dc(T t) {
  auto g = [](auto x) -> decltype(auto) { return x; };
  return g(t);
}
int main() {
  return dflt<int>() + dflt<long>() + two<char>(1) + S().m<int>() + C<int>().m() + nt() +
         outer(1) + outer(2L) + dc(3);
}
