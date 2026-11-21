//@ expect: compile-fail
//@ strict-diags
//@ skip(clang): neither clang 22 nor GCC 16.1 implements P2686R5 constexpr references to automatic objects
//@ diag: 83:23 error "static assertion expression is not an integral constant expression"
//@ diag: 83:24 note "initializer of 'p' is not a constant expression"
//@ diag: 81:24 note "declared here"
//@ diag: 84:23 error "static assertion expression is not an integral constant expression"
//@ diag: 84:23 note "initializer of 'p' is not a constant expression"
//@ diag: 81:24 note "declared here"
//@ diag: 85:23 error "constexpr variable 'v' must be initialized by a constant expression"
//@ diag: 85:28 note "initializer of 'p' is not a constant expression"
//@ diag: 81:24 note "declared here"
//@ diag: 86:29 error "static assertion expression is not an integral constant expression"
//@ diag: 86:30 note "initializer of 'p' is not a constant expression"
//@ diag: 81:24 note "declared here"
//@ diag: 87:23 error "constexpr if condition is not a constant expression"
//@ diag: 87:24 note "initializer of 'p' is not a constant expression"
//@ diag: 81:24 note "declared here"
//@ diag: 92:23 error "static assertion expression is not an integral constant expression"
//@ diag: 92:24 note "initializer of 'p' is not a constant expression"
//@ diag: 91:24 note "declared here"
//@ diag: 99:23 error "static assertion expression is not an integral constant expression"
//@ diag: 99:24 note "initializer of 'p' is not a constant expression"
//@ diag: 96:24 note "declared here"
//@ diag: 100:23 error "static assertion expression is not an integral constant expression"
//@ diag: 100:23 note "initializer of 'r' is not a constant expression"
//@ diag: 97:24 note "declared here"
//@ diag: 101:23 error "static assertion expression is not an integral constant expression"
//@ diag: 101:23 note "initializer of 'r' is not a constant expression"
//@ diag: 97:24 note "declared here"
//@ diag: 102:23 error "static assertion expression is not an integral constant expression"
//@ diag: 102:23 note "initializer of 't' is not a constant expression"
//@ diag: 98:24 note "declared here"
//@ diag: 108:23 error "static assertion expression is not an integral constant expression"
//@ diag: 108:23 note "initializer of '[a, b]' is not a constant expression"
//@ diag: 106:19 note "declared here"
//@ diag: 109:23 error "static assertion expression is not an integral constant expression"
//@ diag: 109:23 note "initializer of '[a, b]' is not a constant expression"
//@ diag: 106:19 note "declared here"
//@ diag: 112:23 error "static assertion expression is not an integral constant expression"
//@ diag: 112:23 note "initializer of 'd' is not a constant expression"
//@ diag: 111:23 note "declared here"
//@ diag: 117:23 error "static assertion expression is not an integral constant expression"
//@ diag: 117:25 note "initializer of 'a' is not a constant expression"
//@ diag: 115:15 note "declared here"
// [expr.const]/9-10 and [expr.const.init]/2-3 (C++26, P2686R5): an object
// is usable in constant expressions at a point only if it is
// constexpr-representable there, and an automatic object is
// constexpr-referenceable only from its own function parameter scope. In
// a lambda body (another function parameter scope) a constexpr local of
// the enclosing function whose value points to or refers to an automatic
// object, a structured binding of a reference to one, or a tuple-like
// binding whose holding variable is such a reference, is therefore not
// usable: naming it odr-uses it, and reading it in a `static_assert` or a
// constant initializer of the body, by-reference capture or by-copy,
// fails with "initializer of 'p' is not a constant expression" (the
// working draft's example in [expr.const]: `[&] { static_assert(a.r !=
// nullptr); }` is an error). WP-1831.
namespace std {
using size_t = decltype(sizeof 0);
template <class T> struct tuple_size;
template <size_t I, class T> struct tuple_element;
template <class T> requires requires { tuple_size<T>::value; }
struct tuple_size<const T> { static constexpr size_t value = tuple_size<T>::value; };
template <size_t I, class T> struct tuple_element<I, const T> { using type = const typename tuple_element<I, T>::type; };
} // namespace std
struct S { int x, y; };
struct X { template <std::size_t I> constexpr int get() const { return int(I) + 10; } };
template <> struct std::tuple_size<X> { static constexpr size_t value = 2; };
template <std::size_t I> struct std::tuple_element<I, X> { using type = int; };
struct L {
  int m[2];
  template <std::size_t I> constexpr const int &get() const { return m[I]; }
};
template <> struct std::tuple_size<L> { static constexpr size_t value = 2; };
template <std::size_t I> struct std::tuple_element<I, L> { using type = const int &; };
struct A { int *const &r; };

void binding_pointer() {
  constexpr auto [a, b] = S{1, 2};
  constexpr const int *p = &b;
  static_assert(*p == 2);
  [&] { static_assert(*p == 2); }();
  [&] { static_assert(p == &b); }();
  [&] { constexpr int v = *p; (void)v; }();
  [&] { [&] { static_assert(*p == 2); }(); }();
  [&] { if constexpr (*p == 2) {} }();
}
void tuple_binding_pointer() {
  constexpr auto [a, b] = X{};
  constexpr const int *p = &b;
  [&] { static_assert(*p == 11); }();
}
void local_pointer_and_reference() {
  constexpr int v = 2;
  constexpr const int *p = &v;
  constexpr const int &r = v;
  constexpr const int &t = 1;
  [&] { static_assert(*p == 2); }();
  [&] { static_assert(r == 2); }();
  [=] { static_assert(r == 2); }();
  [&] { static_assert(t == 1); }();
}
void reference_bindings() {
  constexpr S cl{1, 2};
  constexpr auto &[a, b] = cl;
  static_assert(b == 2);
  [&] { static_assert(b == 2); }();
  [=] { static_assert(b == 2); }();
  constexpr L ll{{3, 4}};
  constexpr auto &[c, d] = ll;
  [&] { static_assert(d == 4); }();
}
void draft_example(int x) {
  constexpr A a = {&x};
  static_assert(a.r == &x);
  [&] { static_assert(a.r != nullptr); }();
}
