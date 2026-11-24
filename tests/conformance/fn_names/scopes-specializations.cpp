// Function-name spans (spec/ast.md §24.4): a class template
// specialization in the scope names the specialization, its canonical
// arguments one depth in (nested template arguments one depth per
// level); the bracketed list binds the pattern's parameters.
//@ fn-names: scopes-specializations.names
//@ std: c++20
namespace ns {
struct S {};
template <class T> struct C {
  void f(T) { (void)__PRETTY_FUNCTION__; }
  template <class U> U g(U, T *) { (void)__PRETTY_FUNCTION__; return U(); }
  struct In { static void m() { (void)__PRETTY_FUNCTION__; } };
};
template <class T, class U> struct P {};
template <class T> struct P<T, int> { static void p() { (void)__PRETTY_FUNCTION__; } };
}
void use() {
  ns::C<ns::S>{}.f({});
  ns::C<ns::C<int>>{}.f({});
  ns::C<ns::S>{}.g(1L, nullptr);
  ns::C<ns::S>::In::m();
  ns::P<ns::S *, int>::p();
}
