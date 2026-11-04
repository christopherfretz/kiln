// [namespace.udecl]/14: in a non-template class a member function or
// member function template hides the base one a using-declarator
// introduced when the name, parameter-type-list, cv- and ref-qualifiers
// (and template head) match; the return type does not take part
// (`M& operator=(int)` hides `B& operator=(int)`), and it does not matter
// whether the member is declared before or after the using-declarator,
// nor that the using-declarator names an indirect base. WP-612.
//@ expect: run
//@ strict-diags
template <class D> struct DenseBase { int v; };
struct X : DenseBase<X> {};
struct B {
  int v = 0;
  template <class O> int f(const DenseBase<O>&) { return 1; }
  int g(int) const { return 1; }
  B& operator=(int) { v = 1; return *this; }
  template <class O> int t(O) { return 1; }
};
struct M : B {
  using B::f;
  using B::g;
  using B::operator=;
  template <class O> int f(const DenseBase<O>&) { return 2; }
  int g(int) const { return 2; }
  M& operator=(int) { v = 2; return *this; }
  template <class O> long t(O) { return 2; }
  using B::t;
};
struct Mid : B {};
struct Leaf : Mid {
  using B::g;
  int g(int) const { return 3; }
};
int main() {
  M m;
  X x;
  if (m.f(x) != 2) return 1;
  if (m.g(0) != 2) return 2;
  m = 0;
  if (m.v != 2) return 3;
  if (m.t(0) != 2) return 4;
  Leaf l;
  if (l.g(0) != 3) return 5;
  return 0;
}
