//@ args: -O1 -Xclang -disable-llvm-passes
// [temp.explicit]/12,14: after an explicit instantiation declaration only
// inline functions (and functions with a deduced return type) are
// implicitly instantiated; at -O1 clang emits those used ones as
// `available_externally` definitions so they can be inlined (clang
// `basicGVALinkageForFunction`, `shouldEmitFunction`). A `noinline` one,
// and every non-inline member, stays a declaration; a static local of an
// emitted one is `linkonce_odr` in its comdat. The complete-object
// structors are replaced by the base-object ones (Itanium C++ ABI 5.1.4).
template <class T> struct X {
  T v;
  T get() const { return v; }
  T inl();
  T noninl();
  T withstatic() { static T s; return ++s; }
  X();
  ~X() {}
};
template <class T> inline T X<T>::inl() { return v + 1; }
template <class T> T X<T>::noninl() { return v + 2; }
template <class T> X<T>::X() : v() {}
extern template struct X<int>;
int use(X<int>& x) { return x.get() + x.inl() + x.noninl() + x.withstatic(); }
struct B { ~B(); int b; };
template <class T> struct Y : B {
  T v;
  Y() : v() {}
  ~Y() {}
  __attribute__((noinline)) T ni() { return v; }
  constexpr T ce() const { return v * 2; }
  template <class U> U tm(U u) { return u + v; }
  static T sget() { static T s = T(3); return s; }
  struct N { T n; T get() { return n; } };
};
extern template struct Y<int>;
int use2(Y<int>& y, Y<int>::N& n) {
  return y.ni() + y.ce() + y.tm(2L) + Y<int>::sget() + n.get();
}
void use3(Y<int>* p) { p->~Y(); }
Y<int>* mk() { return new Y<int>; }
template <class T> struct Z {
  T v;
  auto ded();
  decltype(auto) ded2() { return v; }
};
template <class T> auto Z<T>::ded() { return v + 1; }
extern template struct Z<int>;
template <class T> inline T fr(T t) { return t; }
template <class T> T fn(T t) { return t + 1; }
extern template int fr(int);
extern template int fn(int);
int use4(Z<int>& z) { return z.ded() + z.ded2() + fr(1) + fn(2); }
