// [dcl.meaning]/1, [class.ctor]/1, [class.dtor]/1: out-of-line constructors,
// destructors and conversion functions of a class template, declared with a
// template-id nested-name-specifier `S<C>::` (WP-141: the declarator scan
// did not recognise `S<C>::S(` as a constructor declarator).
//@ expect: run
template <class C> struct S {
  int v;
  S(int);
  S(const S &);
  ~S();
  operator int() const;
};
template <class C> S<C>::S(int x) : v(x) {}
template <class C> S<C>::S(const S &o) : v(o.v + 1) {}
template <class C> S<C>::~S() {}
template <class C> S<C>::operator int() const { return v; }
template struct S<char>;
int main() {
  S<int> a(3);
  S<int> b(a);
  return int(b) - 4;
}
