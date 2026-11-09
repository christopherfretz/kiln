// clang's `annotate` attribute on templates (WP-1113,
// [dcl.attr.grammar]/5, [temp.inst]/3): each instantiated function,
// variable, static data member and member function is annotated as a
// non-template one would be, as are a class's constructors and
// destructors (the base-object variants) and an out-of-line virtual
// function (not its thunk).
template <class T> [[clang::annotate("tmpl")]] T tf(T t) {
  T l [[clang::annotate("tloc", sizeof(T))]] = t;
  static T s [[clang::annotate("tst")]];
  s = l;
  return l;
}
int k() { return tf(1) + tf<long>(2); }
template <class T> struct TS {
  [[clang::annotate("tsm")]] void m() {}
  static inline int sv [[clang::annotate("tsv")]] = 1;
  T fld [[clang::annotate("tsfld")]];
};
int tsu() { TS<int> t; t.m(); t.fld = 2; return TS<long>::sv; }
template <class T> T tv [[clang::annotate("tv")]] = T(1);
int tvu() { return tv<int>; }
template <int N> struct TA {
  [[clang::annotate("targ", N)]] static int get() { return N; }
};
int tau() { return TA<1>::get() + TA<2>::get() + TA<1>::get(); }
struct V { virtual void vf(); };
struct V2 { virtual void vg(); };
struct D : V2, V { [[clang::annotate("vf")]] void vf() override; };
void D::vf() {}
struct K { [[clang::annotate("ctor")]] K(); [[clang::annotate("dtor")]] ~K(); };
K::K() {}
K::~K() {}
void usek() { K k; }
