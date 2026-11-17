// [temp.names]/7.3.4, [temp.res.general]/4: the terminal name of a
// qualified name in a type-only context (a member declaration's
// decl-specifier, a typename-specifier, a parameter of a member function,
// an alias-declaration's defining-type-id) followed by `<` starts a
// template argument list without `template`: `A<U>::SubA<int> m;`.
// Clang 22 asks for 'template' ("use 'template' keyword to treat 'SubA'
// as a dependent template name") and GCC 16.1 reports "non-template
// 'SubA' used as template": both keep the rule before P1787R6. Found
// running GCC's C++ testsuite (`template/nested3`; WP-1546).
//@ expect: run
//@ std: c++20
//@ skip(clang): clang 22 requires 'template' before a member template in a type-only context
//@ strict-diags
template <class T1> struct A {
  template <class S> struct SubA { int k = 1; };
};
template <class U> struct B {
  A<U>::SubA<int> a;
  typename A<U>::SubA<short> b;
  using C = A<U>::SubA<char>;
  int f(A<U>::SubA<long> p) { return p.k; }
};
int main() {
  B<char> x;
  return x.a.k + x.b.k + B<char>::C().k + x.f({}) == 4 ? 0 : 1;
}
