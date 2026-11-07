// GNU `noreturn` on a member of a class template survives instantiation
// together with the rest of the function type (clang's
// `TransformFunctionProtoType` keeps the `ExtInfo`): `&S<true>::e` has
// the type of `&die`, `noexcept` included, a dependent `noexcept(B)` is
// substituted, and calls to them end control flow (no -Wreturn-type).
// kiln dropped the bit from the instantiated member's type.
//@ expect: compile-ok
//@ strict-diags
//@ std: c++17
template <class A, class B> constexpr bool same = false;
template <class A> constexpr bool same<A, A> = true;
typedef void (*NRT)() noexcept __attribute__((noreturn));
typedef void (*NRF)() __attribute__((noreturn));
template <bool B> struct S {
  static void e() noexcept __attribute__((noreturn));
  static void f() noexcept(B) __attribute__((noreturn));
  [[gnu::noreturn]] void g() const & noexcept;
  template <class T> static void t(T) noexcept __attribute__((noreturn));
};
template <bool B> void S<B>::f() noexcept(B) { __builtin_trap(); }
typedef void (S<true>::*NRM)() const & noexcept __attribute__((noreturn));
static void die() noexcept __attribute__((noreturn));
static void die() noexcept { __builtin_trap(); }
static_assert(same<decltype(&die), NRT>);
static_assert(same<decltype(&S<true>::e), NRT>);
static_assert(same<decltype(&S<true>::f), NRT>);
static_assert(same<decltype(&S<false>::f), NRF>);
static_assert(same<decltype(&S<true>::g), NRM>);
typedef void (*NRI)(int) noexcept __attribute__((noreturn));
static_assert(same<decltype(&S<true>::t<int>), NRI>);
static_assert(noexcept(S<true>::e()));
int r1() { S<false>::e(); }
int r2() {
  auto p = &S<true>::f;
  p();
}
int r3() { S<true>::t(1); }
