// GNU `__restrict` applied to a reference type (through a typedef or a
// template argument; written on a reference declarator it is the same)
// is kept: `int &__restrict` is a different type from `int &`, as in
// clang 22 and GCC 16.1 (both mangle a template argument of that type
// `rRi`, `_Z1gIrRiEvT_`). [dcl.ref]/1 ignores only cv-qualifiers applied
// to a reference through a typedef or a template argument.
// A top-level `__restrict` on a parameter is still dropped from the
// function type ([dcl.fct]/5), so both declarations of `f` declare one
// function. Found comparing restrict parameters with clang.
//@ xfail(kiln): WP-1668 follow-up, TypeTable::qualify drops every qualifier applied to a reference type, `__restrict` included
//@ oracle: gcc
template <class A, class B> struct same { static constexpr bool value = false; };
template <class A> struct same<A, A> { static constexpr bool value = true; };

typedef int &R;
typedef R __restrict RR;
static_assert(!same<RR, R>::value, "int &__restrict is not int &");
static_assert(same<RR, int &__restrict>::value, "one type, however spelled");

template <class T> struct Tag { static constexpr int v = 0; };
template <> struct Tag<int &__restrict> { static constexpr int v = 1; };
static_assert(Tag<R>::v == 0 && Tag<RR>::v == 1, "a distinct specialization");

void f(int &);
void f(int &__restrict a) { a = 1; }
void call(int &x) { f(x); }
