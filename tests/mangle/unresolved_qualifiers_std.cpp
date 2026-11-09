// Mangling differential: non-member unresolved names and substitutions
// (spec/abi.md §8.15, WP-1061; [temp.over.link]/5): `std` is spelled
// `3std` in an unresolved qualifier (no `St`), a qualifier repeated in
// the signature is not a substitution candidate inside `sr` but its
// template arguments are (`IS0_E`), and `gs` survives before nested
// levels (`gssr3std1XIT_E1YE1g`).
namespace std { template<class T> struct X { static int g(T); static constexpr int v = 0; struct Y { static int g(T); }; }; int g(int); long g(long); namespace M { int g(int); long g(long);} }
namespace N { template<class T> struct X { static int g(T); static constexpr int v = 0; struct Y { static int g(T); }; }; }
template<int> struct S {};
template<class T> auto f1(T t) -> decltype(std::X<T>::g(t)) { return 0; }
template<class T> auto f2(T t) -> decltype(std::g(t)) { return 0; }
template<class T> auto f3(T t) -> decltype(std::M::g(t)) { return 0; }
template<class T> auto f4(T t) -> decltype(N::X<T>::g(t) + N::X<T>::g(t)) { return 0; }
template<class T> auto f5(N::X<T>, T t) -> decltype(N::X<T>::g(t)) { return 0; }
template<class T> auto f6(T t) -> decltype(N::X<T>::Y::g(t) + N::X<T>::g(t)) { return 0; }
template<class T> auto f7(T t, S<N::X<T>::v>) -> decltype(N::X<T>::g(t)) { return 0; }
template<class T> auto f8(T t) -> decltype(::std::X<T>::Y::g(t)) { return 0; }
template<class T> auto f9(T t) -> decltype(std::X<T>::v + std::X<T>::Y::g(t)) { return 0; }
template<class T> auto f10(T t) -> decltype(::std::g(t)) { return 0; }
template<class T> auto f11(T t) noexcept(noexcept(N::X<T>::g(t))) -> int { return 0; }
template<class T> auto f12(T t) -> decltype(N::X<decltype(N::X<T>::g(t))>::g(1)) { return 0; }
void use() {
  f1(1); f2(1); f3(1); f4(1); f5(N::X<int>(), 1); f6(1); f7<int>(1, S<0>()); f8(1); f9(1); f10(1); f11(1); f12(1);
}
