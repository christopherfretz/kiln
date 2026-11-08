// [temp.deduct.general]/7, [temp.deduct.call]/10 (CWG1391): the
// explicitly specified template arguments are substituted into the
// function type, its return type included, before the arguments are
// checked against the parameters they made non-dependent. With every
// template argument given, `conv(const Str<I, A>&)` drops out on its
// `enable_if` return type, so the conversion from `const char*` to
// `Str<char, Al<wchar_t>>` (whose instantiation is ill-formed) is never
// tried (clang `SubstituteExplicitTemplateArguments`; Boost.Locale
// `utf_to_utf<wchar_t, char, Alloc>(const char*, const char*)`, WP-804).
//@ expect: run
//@ strict-diags
//@ stdout: 1 2 1

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
if_for<A, I, int> conv(const Str<I, A>&) { return 2; }

int main() {
  const char* s = "x";
  Str<char, Al<char>> t(s);
  std::printf("%d %d %d\n", conv<wchar_t, char, Al<wchar_t>>(s), conv<wchar_t>(t),
              conv<wchar_t, char, Al<char>>(s));
}
