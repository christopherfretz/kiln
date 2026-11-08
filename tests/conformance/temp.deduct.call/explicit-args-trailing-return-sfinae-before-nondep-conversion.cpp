// As explicit-args-return-sfinae-before-nondep-conversion.cpp, with the
// constraining return type written after the parameters: clang
// substitutes the explicit arguments into the parameters and then into
// the trailing return type before any argument conversion, so the
// candidate drops out without instantiating `Str<char, Al<wchar_t>>`.
//@ expect: run
//@ strict-diags
//@ stdout: 1
//@ xfail(kiln): WP-804 follow-up, a trailing return type is substituted only with the deduced arguments, after the CWG1391 conversion check instantiates the parameter's class

#include <cstdio>
#include <type_traits>

template <class C, class A> struct Str {
  static_assert(std::is_same<C, typename A::value_type>::value, "mismatch");
  Str(const C*) {}
};
template <class T> struct Al { using value_type = T; };
template <class A, class T, class R = void>
using if_for = typename std::enable_if<std::is_same<typename A::value_type, T>::value, R>::type;

template <class O, class I, class A = Al<O>>
int conv(const I*) { return 1; }
template <class O, class I, class A>
auto conv(const Str<I, A>&) -> if_for<A, I, int> { return 2; }

int main() {
  const char* s = "x";
  std::printf("%d\n", conv<wchar_t, char, Al<wchar_t>>(s));
}
