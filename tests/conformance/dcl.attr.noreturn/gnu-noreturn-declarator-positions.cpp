// [dcl.decl.general]/4, [dcl.ptr]/1, [dcl.attr.noreturn] (GNU form): GNU
// `noreturn` written after the `(` of a grouping paren or after a `*`, in
// a declarator or a type-id, makes the declared function type noreturn
// (clang's `ParseParenDeclarator` eats the attributes before deciding
// between grouping and a parameter list). kiln dropped the attribute
// there, and took `void g(void (__attribute__((noreturn)) *p)(int))` for
// a parameter list (WP-708).
//@ std: c++17
//@ expect: compile-ok
//@ strict-diags
template <class A, class B> struct same { static const bool v = false; };
template <class A> struct same<A, A> { static const bool v = true; };
typedef void NRF(int) __attribute__((noreturn));
typedef NRF *NRP;
void (__attribute__((noreturn)) *fp1)(int);
void (*__attribute__((noreturn)) fp2)(int);
void (*fp3)(int) __attribute__((noreturn));
using F = void (*__attribute__((noreturn)))(int);
typedef void (__attribute__((noreturn)) *T1)(int);
typedef void (*__attribute__((noreturn)) T2)(int);
static_assert(same<decltype(fp1), NRP>::v, "fp1");
static_assert(same<decltype(fp2), NRP>::v, "fp2");
static_assert(same<decltype(fp3), NRP>::v, "fp3");
static_assert(same<F, NRP>::v, "F");
static_assert(same<T1, NRP>::v, "T1");
static_assert(same<T2, NRP>::v, "T2");
struct C { void (__attribute__((noreturn)) *m)(int); };
static_assert(same<decltype(C::m), NRP>::v, "m");
void g(void (__attribute__((noreturn)) *p)(int));
static_assert(same<decltype(g), void(NRP)>::v, "g");
void h(void (__attribute__((noreturn)) *)(int));
static_assert(same<decltype(h), void(NRP)>::v, "h");
// Not a grouping paren: the attribute starts a parameter list.
void k(int (__attribute__((unused)) int x));
static_assert(same<decltype(k), void(int (*)(int))>::v, "k");
static_assert(same<void (__attribute__((noreturn)) *)(int), NRP>::v, "type-id");
template <class T> struct W { using P = void (__attribute__((noreturn)) *)(T); };
static_assert(same<W<int>::P, NRP>::v, "W");
static_assert(!same<void (*)(int), NRP>::v, "plain");
