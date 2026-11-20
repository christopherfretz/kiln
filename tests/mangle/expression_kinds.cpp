// Mangling differential: expression kinds in dependent signature
// positions ([temp.over.link]/5, Itanium C++ ABI 5.1.6 <expression>;
// spec/abi.md §8.15, WP-1736), each as clang 19/22 `mangleExpression`
// encodes it. Each `// demangles:` line is what llvm-cxxfilt (or GNU
// c++filt, for the names outside LLVM's demangler) prints for one of
// kiln's names: the round trip mangle_vs_clang checks.
// - `sizeof...` of a function parameter pack: `sZ <function-param>`;
// demangles: A<sizeof... (fp)>::type fp_pack<int, int>(int, int)
// demangles: decltype(sizeof... (fp)) fp_dt<int>(int)
// - `sizeof...` of a pack partially substituted through an alias
//   template: `sP <template-arg>* E`;
// demangles: void partial<char>(A<sizeof... (int, char)>)
// - a `decltype` qualifier: an unresolved type, no closing `E` (clang
//   spells a class there as a plain name, which only GNU c++filt reads);
// demangles: decltype (S::g({parm#1})) dq<int>(int)
// - a rewritten comparison: the written operator (`ne`, `lt`);
// demangles: decltype(fp, X{} != X{}) rw<int>(int)
// demangles: decltype(fp, Y{} < Y{}) rw2<int>(int)
// - designated initializers: `di <source-name> <expression>`;
// demangles: decltype(k({.a = fp, .b = 2})) desig<int>(int)
// - a function parameter pack of the class expanded in a member
//   template's pattern: `v110_SUBSTPACK <function-param>`;
// demangles: L<decltype(k2(operator _SUBSTPACK, fp, fp1))...> Q<int, long>::h<int, int>(int, long, int, int)
// - a `decltype` whose operand only names a non-dependent parameter
//   is not dependent: the type (`i`), not `DTplfp_Li1EE`.
// demangles: int N::nd<int>(int, int)
// demangles: int NT<int>::nd<int>(int, int)
template <unsigned long N> struct A {
  typedef int type;
};
template <class... T> auto fp_pack(T... a) -> typename A<sizeof...(a)>::type { return 0; }
template <class... T> auto fp_dt(T... a) -> decltype(sizeof...(a)) { return 0; }

template <class... T> using Al = A<sizeof...(T)>;
template <class... U> void partial(Al<int, U...>) {}

struct S {
  template <class T> static T g(T t) { return t; }
};
S s;
template <class T> auto dq(T t) -> decltype(decltype(s)::g(t)) { return t; }
inline auto ll = [](auto...) {};
template <class I, class Ar> void mm(void (I::*)(Ar) const) {}
template <class T> auto dql() -> decltype(mm(&decltype(ll)::operator()<T>)) {}

struct X {
  bool operator==(const X &) const { return true; }
};
struct Y {
  int operator<=>(const Y &) const { return 0; }
};
template <class T> auto rw(T t) -> decltype(t, X{} != X{}) { return false; }
template <class T> auto rw2(T t) -> decltype(t, Y{} < Y{}) { return false; }

struct P {
  int a, b;
};
inline int k(P p) { return p.a; }
template <class T> auto desig(T t) -> decltype(k({.a = t, .b = 2})) { return 0; }

template <class Ta, class Tb> int k2(Ta, Tb) { return 0; }
template <class... Z> struct L {};
template <class... T> struct Q {
  template <class... U> static auto h(T... t, U... u) -> L<decltype(k2(t, u))...> { return {}; }
};

struct N {
  template <class U> static auto nd(int t, U) -> decltype(t + 1) { return t; }
};
template <class T> struct NT {
  template <class U> static auto nd(T t, U) -> decltype(t + 1) { return t; }
};

int use() {
  fp_pack(1, 2);
  fp_dt(1);
  partial<char>(A<2>{});
  dq(1);
  dql<int>();
  rw(1);
  rw2(1);
  desig(1);
  Q<int, long>::h(1, 2L, 3, 4);
  N::nd(1, 2);
  NT<int>::nd(1, 2);
  return 0;
}
