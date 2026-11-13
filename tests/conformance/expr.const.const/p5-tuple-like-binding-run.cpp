// [expr.const.const]/2, /5.1, [expr.const.imm]/2 (P4101R1), [dcl.struct.bind]/7:
// the bindings of a `constexpr` tuple-like structured binding (`get<I>()`
// members) of an object holding reflections are references to immediate
// objects; reading one in a manifestly constant-evaluated context, an
// immediate function, or a function template specialization used only
// there is well-formed, and a dependent static data member reached
// through `this->` is too when the member function is consteval.
//@ expect: run
//@ oracle: gcc
//@ args: -freflection
//@ skip(clang): clang 19/22 do not implement P2996 reflection
//@ stdout: ok
extern "C" int printf(const char *, ...);
using info = decltype(^^int);
namespace std {
template <class> struct tuple_size;
template <decltype(sizeof 0), class> struct tuple_element;
}  // namespace std
struct P {
  info a, b;
  template <decltype(sizeof 0) I> consteval info get() const { return I == 0 ? a : b; }
};
template <> struct std::tuple_size<P> { static constexpr decltype(sizeof 0) value = 2; };
template <decltype(sizeof 0) I> struct std::tuple_element<I, P> { using type = info; };

constexpr P p{^^int, ^^char};
constexpr auto [x, y] = p;
constexpr info cx = x;
consteval info first() { return x; }
static_assert((cx == ^^int) && (first() == ^^int) && (y == ^^char));

template <class T> consteval bool tc() {
  constexpr auto [a, b] = T{^^int, ^^char};
  return (a == ^^int) && (b == ^^char);
}
static_assert(tc<P>());

template <class T> struct HS {
  static constexpr T sm{^^long, ^^short};
  consteval info f() const { return this->sm.a; }
};
static_assert(HS<P>{}.f() == ^^long);

int main() { printf("ok\n"); }
