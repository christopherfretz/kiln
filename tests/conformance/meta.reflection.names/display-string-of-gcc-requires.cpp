// [meta.reflection.names]/6-7 (P2996R13), [temp.pre]/9, [dcl.decl.general]/3:
// display_string_of a constrained class, function, variable or alias
// template prints its associated constraints as GCC 16.1's `%D`
// (spec/reflection.md §3.9): the template parameters' type-constraints
// in order (a constrained pack as the fold `(... && C<Ts>)`), then the
// requires-clause, as one `&&` chain after the header with GCC's padding
// (`template<class T>  requires  C<T> struct X`); a trailing
// requires-clause follows the declarator, before `[with ...]`. Operands
// print with C precedence: a binary operator's operands bare at its own
// level, any template-id, concept-id or `T::v` operand parenthesized;
// `sizeof (T)`, `sizeof ... (Ts ...)`, a conditional without its colon.
// The reference workload compares these strings against GCC's.
//@ expect: compile-ok
//@ oracle: gcc
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
#include <concepts>
#include <type_traits>
#ifdef __KILN__
#include "kiln_meta.h"
#else
#include <meta>
#endif
using namespace std::meta;

namespace demo {
constexpr bool flag = true;
template <class T> concept Cn = std::is_integral_v<T>;
// The reduced reference-workload case.
template <class T> requires std::is_integral_v<T> struct OnlyInt {};
template <std::integral T> struct TC {};
template <class T> requires (sizeof(T) > 4) struct Big {};
template <class T> requires std::integral<T> && std::is_signed_v<T> struct And {};
template <class T> requires std::integral<T> || std::floating_point<T> struct Or {};
template <std::integral T, class U> requires std::is_same_v<T, U> struct Mixed {};
template <class T> requires true struct Lit {};
template <class T> requires (!std::is_integral_v<T>) struct Not {};
template <class T> requires Cn<T> struct User {};
template <class T> requires T::value struct Dep {};
template <class T> requires std::is_integral_v<typename T::type> struct Nested {};
template <class T> requires flag struct Var {};
template <class T> requires (std::integral<T> || std::is_enum_v<T>) && true struct Group {};
template <class T> requires std::is_convertible_v<T, const int&> struct Ref {};
template <std::integral T, std::floating_point U> struct Two {};
template <std::convertible_to<int> T> struct Conv {};
template <class T> requires (1 + 2 > 0) struct Arith {};
template <int N> requires (N > 0) && (N < 10) struct Range {};
template <int N> requires ((N + 1) * 2 > 0) struct Prec {};
template <int N> requires (!(N > 0)) struct NotRel {};
template <int N> requires (-N < 0) struct Neg {};
template <bool X> requires (!X) struct NotX {};
template <int N> requires (N % 2 == 0) struct Even {};
template <class T> requires (sizeof(T) == sizeof(int)) struct Same {};
template <class T> requires (alignof(T) >= 4) struct Align {};
template <class T> requires (std::integral<T> || (std::is_enum_v<T> && std::is_signed_v<T>)) struct Inner {};
// Packs and folds.
template <std::integral... Ts> struct Pack {};
template <std::integral U, std::integral... Ts> struct Pack2 {};
template <class... Ts> requires (!(std::integral<Ts> && ...)) struct NotFold {};
template <int... N> requires ((N + ... + 0) > 0) struct BinFold {};
template <class... Ts> requires (std::integral<Ts> || ...) struct RFold {};
template <class... Ts> requires (sizeof...(Ts) > 0 && sizeof...(Ts) < 3) struct Count {};
// Non-type placeholders, decltype, conditionals.
template <std::integral auto N> struct Auto {};
template <class T, std::integral auto N> struct Auto2 {};
template <class T> requires std::is_same_v<decltype(T::x), int> struct Decl {};
template <class T> requires (sizeof(T) > 1 ? sizeof(T) < 4 : sizeof(T) == 1) struct Cond {};
template <class T> requires (sizeof(T) > 1 ? true : false) || false struct Cond2 {};
// Requires-expressions.
template <class T> requires requires { typename T::type; } struct RType {};
template <class T> requires requires (const T& a, T b) { a + b; a - b; } struct RSimple {};
template <class T> requires requires (T t) { { t + t } -> std::same_as<T>; } struct RComp {};
template <class T> requires requires (T t) { { t + t } noexcept -> std::convertible_to<long>; } struct RNoex {};
template <class T> requires requires (T t) { requires sizeof(T) > 1; } struct RNested {};
// Function, variable and alias templates; trailing requires-clauses.
template <class T> requires std::is_integral_v<T> void f(T) {}
template <class T> void g(T) requires std::is_integral_v<T> {}
template <class T> void g2(T) requires std::integral<T> && (sizeof(T) > 1) {}
template <std::integral T> void h(T) requires (sizeof(T) > 1) {}
template <class T> requires std::is_integral_v<T> constexpr int v = 0;
template <class T> requires (!std::integral<T>) constexpr bool nv = true;
template <class T> requires std::integral<T> using AT = T;
struct X { template <class T> requires std::integral<T> void m(T); };
template <class T> requires std::integral<T> struct E {
  void m() requires (sizeof(T) > 1) {}
  template <class U> void n(U) requires std::same_as<T, U> {}
};
template <class T> struct Q {
  template <class U> requires std::integral<U> void m(U) requires std::same_as<T, int> {}
};
}  // namespace demo

consteval bool eq(std::string_view a, const char* b) {
  decltype(sizeof 0) n = 0;
  while (b[n]) {
    if (n >= a.size() || a.data()[n] != b[n]) return false;
    ++n;
  }
  return n == a.size();
}
#define CHECK(r, s) static_assert(eq(display_string_of(r), s))

CHECK(^^demo::OnlyInt, "template<class T>  requires  is_integral_v<T> struct demo::OnlyInt");
CHECK(^^demo::TC, "template<class T>  requires  integral<T> struct demo::TC");
CHECK(^^demo::Big, "template<class T>  requires  sizeof (T) > 4 struct demo::Big");
CHECK(^^demo::And, "template<class T>  requires (integral<T>) && (is_signed_v<T>) struct demo::And");
CHECK(^^demo::Or, "template<class T>  requires (integral<T>) || (floating_point<T>) struct demo::Or");
CHECK(^^demo::Mixed,
      "template<class T, class U>  requires (integral<T>) && (is_same_v<T, U>) struct demo::Mixed");
CHECK(^^demo::Lit, "template<class T>  requires  true struct demo::Lit");
CHECK(^^demo::Not, "template<class T>  requires !(is_integral_v<T>) struct demo::Not");
CHECK(^^demo::User, "template<class T>  requires  Cn<T> struct demo::User");
CHECK(^^demo::Dep, "template<class T>  requires  T::value struct demo::Dep");
CHECK(^^demo::Nested, "template<class T>  requires  is_integral_v<typename T::type> struct demo::Nested");
CHECK(^^demo::Var, "template<class T>  requires  demo::flag struct demo::Var");
CHECK(^^demo::Group,
      "template<class T>  requires ((integral<T>) || (is_enum_v<T>)) && true struct demo::Group");
CHECK(^^demo::Ref, "template<class T>  requires  is_convertible_v<T, const int&> struct demo::Ref");
CHECK(^^demo::Two,
      "template<class T, class U>  requires (integral<T>) && (floating_point<U>) struct demo::Two");
CHECK(^^demo::Conv, "template<class T>  requires  convertible_to<T, int> struct demo::Conv");
CHECK(^^demo::Arith, "template<class T>  requires 1 + 2 > 0 struct demo::Arith");
CHECK(^^demo::Range, "template<int N>  requires  N > 0 && N < 10 struct demo::Range");
CHECK(^^demo::Prec, "template<int N>  requires (N + 1) * 2 > 0 struct demo::Prec");
CHECK(^^demo::NotRel, "template<int N>  requires !(N > 0) struct demo::NotRel");
CHECK(^^demo::Neg, "template<int N>  requires - N < 0 struct demo::Neg");
CHECK(^^demo::NotX, "template<bool X>  requires ! X struct demo::NotX");
CHECK(^^demo::Even, "template<int N>  requires  N % 2 == 0 struct demo::Even");
CHECK(^^demo::Same, "template<class T>  requires  sizeof (T) == sizeof (int) struct demo::Same");
CHECK(^^demo::Align, "template<class T>  requires  alignof (T) >= 4 struct demo::Align");
CHECK(^^demo::Inner,
      "template<class T>  requires (integral<T>) || (is_enum_v<T>) && (is_signed_v<T>) struct demo::Inner");

CHECK(^^demo::Pack, "template<class ... Ts>  requires (... && integral<Ts>) struct demo::Pack");
CHECK(^^demo::Pack2,
      "template<class U, class ... Ts>  requires (integral<U>) && ((... && integral<Ts>)) struct demo::Pack2");
CHECK(^^demo::NotFold, "template<class ... Ts>  requires !((integral<Ts> && ...)) struct demo::NotFold");
CHECK(^^demo::BinFold, "template<int ...N>  requires ((N + ... +0)) > 0 struct demo::BinFold");
CHECK(^^demo::RFold, "template<class ... Ts>  requires (integral<Ts> || ...) struct demo::RFold");
CHECK(^^demo::Count,
      "template<class ... Ts>  requires  sizeof ... (Ts ...) > 0 && sizeof ... (Ts ...) < 3 struct demo::Count");

CHECK(^^demo::Auto, "template<auto N>  requires  integral<decltype(N)> struct demo::Auto");
CHECK(^^demo::Auto2, "template<class T, auto N>  requires  integral<decltype(N)> struct demo::Auto2");
CHECK(^^demo::Decl, "template<class T>  requires  is_same_v<decltype(T::x), int> struct demo::Decl");
CHECK(^^demo::Cond,
      "template<class T>  requires  sizeof (T) > 1 ? sizeof (T) < 4 sizeof (T) == 1 struct demo::Cond");
CHECK(^^demo::Cond2, "template<class T>  requires (sizeof (T) > 1 ? true  false) || false struct demo::Cond2");

CHECK(^^demo::RType, "template<class T>  requires requires{typename T::type;} struct demo::RType");
CHECK(^^demo::RSimple,
      "template<class T>  requires requires(const T& a, T b) {a + b;a - b;} struct demo::RSimple");
CHECK(^^demo::RComp,
      "template<class T>  requires requires(T t) {{t + t} -> decltype(auto) [requires "
      "std::same_as<<placeholder>, T>];} struct demo::RComp");
CHECK(^^demo::RNoex,
      "template<class T>  requires requires(T t) {{t + t}noexcept -> decltype(auto) [requires "
      "std::convertible_to<<placeholder>, long int>];} struct demo::RNoex");
CHECK(^^demo::RNested,
      "template<class T>  requires requires(T t) {requires sizeof (T) > 1;} struct demo::RNested");

CHECK(^^demo::f, "template<class T>  requires  is_integral_v<T> void demo::f(T)");
CHECK(^^demo::f<int>, "void demo::f(T) [with T = int]");
CHECK(^^demo::g, "template<class T> void demo::g(T) requires  is_integral_v<T>");
CHECK(^^demo::g<int>, "void demo::g(T) requires  is_integral_v<T> [with T = int]");
CHECK(^^demo::g2, "template<class T> void demo::g2(T) requires (integral<T>) && sizeof (T) > 1");
CHECK(^^demo::h, "template<class T>  requires  integral<T> void demo::h(T) requires  sizeof (T) > 1");
CHECK(^^demo::v, "template<class T>  requires  is_integral_v<T> constexpr const int demo::v<T>");
CHECK(^^demo::nv, "template<class T>  requires !(integral<T>) constexpr const bool demo::nv<T>");
CHECK(^^demo::AT, "template<class T>  requires  integral<T> using demo::AT = T");
CHECK(^^demo::X::m, "template<class T>  requires  integral<T> void demo::X::m(T)");
CHECK(^^demo::E<int>::m, "void demo::E<T>::m() requires  sizeof (T) > 1 [with T = int]");
CHECK(^^demo::E<int>::n,
      "template<class U> void demo::E<T>::n(U) requires  same_as<T, U> [with T = int]");
CHECK(^^demo::E<int>::n<int>,
      "void demo::E<T>::n(U) requires  same_as<T, U> [with U = int; T = int]");
CHECK(^^demo::E<long>, "demo::E<long int>");
CHECK(^^demo::Q<int>::m,
      "template<class U>  requires  integral<U> void demo::Q<T>::m(U) requires  same_as<T, int> [with T = int]");
CHECK(^^demo::Q<int>::m<int>,
      "void demo::Q<T>::m(U) requires  same_as<T, int> [with U = int; T = int]");

int main() {}
