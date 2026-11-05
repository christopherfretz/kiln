// [conv.fctptr], [dcl.fct]/1, [temp.deduct.call] (C++17, P0012R1): the
// exception specification is part of the function type.
// WP-646 feature probe.
//@ expect: compile-ok
//@ std: c++17
//@ strict-diags
void f() noexcept;
void g();
template <class T, class U> struct same { static constexpr bool v = false; };
template <class T> struct same<T, T> { static constexpr bool v = true; };
static_assert(!same<decltype(f), decltype(g)>::v);
void (*p)() = f;
void (*q)() noexcept = f;
template <class R, bool N> constexpr bool deduce(R (*)() noexcept(N)) { return N; }
static_assert(deduce(f) && !deduce(g));
struct S { void m() noexcept; };
void (S::*pm)() = &S::m;
static_assert(noexcept(q()) && !noexcept(p()));
