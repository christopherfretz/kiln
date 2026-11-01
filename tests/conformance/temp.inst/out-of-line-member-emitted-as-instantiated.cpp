// [temp.inst]/4, [temp.pre]/8: a non-inline member function of a class template defined out of
// line is templated; only its instantiations are emitted (an unused one with dependent
// expressions emits nothing)
//@ expect: run
//@ strict-diags
template <class C> struct S {
  C v;
  int g() const;
  int unused() const;
  static int k;
};
template <class C> int S<C>::g() const { return v + k; }
template <class C> int S<C>::unused() const { auto t = v.foo(1, 2); return t; }
template <class C> int S<C>::k = sizeof(C);
int main() { S<int> s{3}; return s.g() == 7 ? 0 : 1; }
