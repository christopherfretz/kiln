// C++17 removed [except.spec]/2's ban on an exception-specification in a
// typedef or alias-declaration ([diff.cpp14.except]): `noexcept` is part
// of the function type ([dcl.fct]/1), so a typedef or alias names the
// non-throwing type, distinct from the potentially-throwing one.
//@ expect: compile-ok
//@ strict-diags
//@ std: c++17
template <class T, class U> struct same { static constexpr bool value = false; };
template <class T> struct same<T, T> { static constexpr bool value = true; };
typedef void (*fp1)() noexcept;
typedef void fn2() noexcept(true);
using fp3 = void (*)() throw();
template <class T> using fp4 = void (*)(T) noexcept;
struct S {
  typedef void (*m1)() noexcept;
  using m2 = void (S::*)() noexcept;
};
static_assert(same<fp1, void (*)() noexcept>::value, "");
static_assert(!same<fp1, void (*)()>::value, "");
static_assert(same<fn2*, fp1>::value, "");
static_assert(same<fp3, fp1>::value, "");
static_assert(same<fp4<int>, void (*)(int) noexcept>::value, "");
static_assert(same<S::m1, fp1>::value, "");
static_assert(!same<S::m2, void (S::*)()>::value, "");
void f() noexcept;
fp1 p = f;
