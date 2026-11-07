// [except.spec]/1, [temp.arg.type], [temp.deduct.call] (C++14,
// pre-P0012): before C++17 the exception specification is not part of
// the function type: `void (*)() noexcept` and `void (*)()` are one type,
// a template argument (explicit or deduced) is the type without it, and
// overloading on it alone is a redefinition; the declared entity's own
// type keeps it, so `noexcept(p())` through a `noexcept` pointer or an
// `auto` copy is true while one through a template parameter is false.
// kiln kept `noexcept` in the type at every standard (WP-751).
//@ expect: compile-fail
//@ strict-diags
//@ std: c++14
//@ diag: 31:6 error "redefinition of 'w'"
//@ diag: 30:6 note "previous definition is here"
void f() noexcept;
void g();
template <class T, class U> struct same { static constexpr bool value = false; };
template <class T> struct same<T, T> { static constexpr bool value = true; };
static_assert(same<void (*)() noexcept, void (*)()>::value, "");
static_assert(same<decltype(&f), decltype(&g)>::value, "");
static_assert(same<decltype(f), void()>::value, "");
template <class F> constexpr bool call(F x) { return noexcept(x()); }
template <class F> constexpr bool call2() { return noexcept(((F)nullptr)()); }
void (*p)() noexcept = f;
auto q = &f;
static_assert(noexcept(p()) && noexcept(q()) && noexcept(f()), "");
static_assert(!noexcept(g()), "");
static_assert(!call2<decltype(&f)>(), "");
static_assert(!call2<void (*)() noexcept>(), "");
template <class T> struct W {};
void w(W<void (*)() noexcept>) {}
void w(W<void (*)()>) {}
