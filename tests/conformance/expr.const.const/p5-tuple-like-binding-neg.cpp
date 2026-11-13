// [expr.const.const]/2, /5.1, [expr.const.imm]/2-3 (P4101R1), [dcl.struct.bind]/7:
// a binding of a `constexpr` tuple-like structured binding (`get<I>()`
// members) of an object holding reflections refers to an immediate
// object, so naming it outside an immediate function context is an error:
// at namespace scope, in a function, and in a function template
// specialization (the binding's value known only once instantiated). So
// is `this->sm` with `sm` a `constexpr` static data member whose
// dependent type becomes one holding reflections (WP-1334).
//@ expect: compile-fail
//@ oracle: gcc
//@ args: -freflection
//@ skip(clang): clang 19/22 do not implement P2996 reflection
//@ strict-diags
//@ diag: 32:6 error "'bad' is initialized with a consteval-only value but is not declared 'constexpr'"
//@ diag: 33:21 error "consteval-only value outside an immediate function context"
//@ diag: 35:54 error "consteval-only value outside an immediate function context"
//@ diag: 40:14 note "in instantiation of function template specialization 'tg<P>' requested here"
//@ diag: 38:61 error "consteval-only value outside an immediate function context"
//@ diag: 40:31 note "in instantiation of member function 'HS<P>::f' requested here"
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
constexpr auto [x, y] = P{^^int, ^^char};
info bad = x;
void g() { info l = y; (void)l; }
template <class T> void tg() {
  constexpr auto [a, b] = T{^^int, ^^char}; info l = b; (void)l;
}
template <class T> struct HS {
  static constexpr T sm{^^long, ^^short}; info f() { return this->sm.a; }
};
void use() { tg<P>(); HS<P>{}.f(); }
