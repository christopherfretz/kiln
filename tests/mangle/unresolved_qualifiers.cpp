// Mangling differential: non-member unresolved names in dependent
// signatures (spec/abi.md §8.15, WP-1061; [temp.over.link]/5): `gs` for a
// global-qualified name, `sr <level>+ E` for namespace qualifiers,
// `srN <type> <level>+ E` for a dependent type qualifier, template
// arguments on the qualifier levels and the name. An expression
// equivalent to one spelled earlier in the TU takes that spelling (`f3`'s
// `::N::g(t)` is `sr1NE1g` after `f2`'s `N::g(t)`, `f16`'s `NA::g(t)`
// likewise).
int g(int); long g(long);
namespace N {
  int g(int); long g(long);
  template<class T> int h(T);
  namespace M { int g(int); long g(long); }
  template<class T> struct X { static int g(T); static int v; template<class U> static int k(U); using type = T; };
  inline namespace I { int q(int); long q(long); }
}
namespace NA = N;
struct A { static int g(A); static long g(long); using type = int; struct B { static int g(A); }; };
template<class T> struct TT { static int g(int); static long g(long); };
template<class T> auto f1(T t) -> decltype(::g(t)) { return 0; }
template<class T> auto f2(T t) -> decltype(N::g(t)) { return 0; }
template<class T> auto f3(T t) -> decltype(::N::g(t)) { return 0; }
template<class T> auto f4(T t) -> decltype(N::M::g(t)) { return 0; }
template<class T> auto f5(T t) -> decltype(::N::M::g(t)) { return 0; }
template<class T> auto f6(T t) -> decltype(T::g(t)) { return 0; }
template<class T> auto f7(T t) -> decltype(T::B::g(t)) { return 0; }
template<class T> auto f8(T t) -> decltype(N::X<int>::g(t)) { return 0; }
template<class T> auto f9(T t) -> decltype(::N::X<int>::g(t)) { return 0; }
template<class T> auto f10(T t) -> decltype(N::X<T>::g(t)) { return 0; }
template<class T> auto f11(T t) -> decltype(::N::X<T>::g(t)) { return 0; }
template<class T> auto f12(T t) -> decltype(N::X<T>::v + t) { return 0; }
template<class T> auto f13(T t) -> decltype(N::h<T>(t)) { return 0; }
template<class T> auto f14(T t) -> decltype(::N::h<int>(t)) { return 0; }
template<class T> auto f15(T t) -> decltype(N::X<T>::template k<int>(t)) { return 0; }
template<class T> auto f16(T t) -> decltype(NA::g(t)) { return 0; }
template<class T> auto f17(T t) -> decltype(N::q(t)) { return 0; }
template<class T> auto f18(T t) -> decltype(N::I::q(t)) { return 0; }
template<class T> auto f19(T t) -> decltype(TT<int>::g(t)) { return 0; }
template<class T> auto f20(T t) -> decltype(::TT<T>::g(t)) { return 0; }
template<class T> auto f21(T t) -> decltype(A::g(t)) { return 0; }
template<class T> auto f22(T t) -> decltype(::A::B::g(t)) { return 0; }
template<class T> auto f23(T t) -> decltype(N::X<typename T::type>::g(1)) { return 0; }
template<class T> auto f24(T t) -> decltype(T::type::g(A())) { return 0; }
template<class T> auto f25(T t) -> decltype(N::template X<T>::g(t)) { return 0; }
template<class T> auto f26(T t) -> decltype(g(t) + N::X<T>::v) { return 0; }
template<class T> auto f27(T t) -> decltype(::N::X<T>::v) { return 0; }
template<class T> auto f28(T t) -> decltype(&N::X<T>::g) { return 0; }
struct G { using type = A; };
void use() {
  f1(1); f2(1); f3(1); f4(1); f5(1); f6(A()); f7(A()); f8(1); f9(1); f10(1); f11(1); f12(1);
  f13(1); f14(1); f15(1); f16(1); f17(1); f18(1); f19(1); f20(1); f21(1); f22(A());
  f23(A()); f24(G()); f25(1); f26(1); f27(1); f28(1);
}
