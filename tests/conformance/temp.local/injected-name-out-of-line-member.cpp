// [temp.local]/1, [basic.scope.class]: in an out-of-line member definition of a class template,
// the template's name used without arguments is the injected-class-name (not CTAD)
//@ expect: run
//@ strict-diags
template <class C> struct S {
  C v;
  S(int a, int b, int c) : v(a + b + c) {}
  int g() const;
  template <class I> int h(I a) const;
  struct N { int k(); };
};
template <class C> int S<C>::g() const { const S t(1, 2, 3); return t.v + v; }
template <class C> template <class I> int S<C>::h(I a) const {
  S t(a, 2, 3);
  S* p = &t;
  return p->v;
}
template <class C> int S<C>::N::k() { S s(0, 0, 4); return s.v; }
int main() {
  S<long> s(1, 1, 1);
  S<long>::N n;
  return s.g() == 9 && s.h(5) == 10 && n.k() == 4 ? 0 : 1;
}
