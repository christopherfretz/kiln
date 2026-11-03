// [temp.point]/7, [basic.start.dynamic]/1: an implicitly instantiated
// variable template specialization or static data member of a class
// template specialization is defined at the end of the translation unit
// (clang PerformPendingInstantiations), after everything else the TU
// emits, in instantiation order; a variable whose initializer requires
// further instantiations is defined after them. Its global, its unordered
// __cxx_global_var_init.N and its llvm.global_ctors entry follow.
int dyn();
struct C { C(); ~C(); int v; };
template <class T> T vt = T(dyn());
template <class T> T cst = T(3);
template <class T> constexpr T cvt = T(3);
template <class T> C vc{};
template <class T> inline int ivt = dyn();
template <class T> int vz;
template <class T> thread_local int tl = dyn();
template <class T> struct S {
  static int sm;
  static inline int ism = dyn();
  static constexpr int csm = 5;
  static const int k;
  static C sc;
  template <class U> static U mvt;
};
template <class T> int S<T>::sm = dyn() + 1;
template <class T> const int S<T>::k = 9;
template <class T> C S<T>::sc;
template <class T> template <class U> U S<T>::mvt = U(dyn());
int before = dyn();
int use1() { return vt<int> + S<int>::sm + cst<int> + vz<int>; }
const long *use2() { return &cvt<long>; }
int use3() { return S<long>::ism + vt<short> + vc<int>.v + ivt<char> + S<int>::k; }
int x = vt<long> + S<char>::mvt<int>;
template <class T> int tf() { return vt<T> + S<T>::sc.v + tl<T>; }
int use5() { return tf<unsigned>(); }
// Nested: d<int> requires c<int>, which requires a<int> and b<int>.
template <class T> T f() { return T(dyn()); }
template <class T> T a = f<T>();
template <class T> T b = T(dyn());
template <class T> T c = a<T> + b<T>;
template <class T> T d = c<T> + 1;
int use6() { return d<int>; }
template <> int b<long> = dyn();
int use7() { return b<long> + sizeof(c<char>); }
int after = dyn();
int use4() { return after + vt<int>; }
