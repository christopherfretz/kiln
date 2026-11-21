// [clang ext] [temp.deduct.general]/8 A parameter qualified with an address
// space in a function type inside a function template's declaration (a
// parameter's type or a trailing return type) is a substitution failure
// in the immediate context, so a requirement naming that specialization
// is not satisfied; the same type is valid for the arguments that do not
// qualify it, and an alias template's function type is substituted
// without its parameters (clang `SubstParmVarDecl`; reduced from real
// code). spec/sema.md §5.8, WP-1823.
//@ expect: compile-ok
//@ strict-diags
#define AS(n) __attribute__((address_space(n)))
template <int N> auto a1() -> void (*)(int AS(N));
template <int N> concept CA1 = requires { a1<N>(); };
static_assert(!CA1<1>);
template <class T> void a2(void (*)(T));
template <class X> concept CA2 = requires { a2<X>(nullptr); };
static_assert(!CA2<int AS(1)>);
static_assert(CA2<int>);
static_assert(CA2<int AS(1) *>);
template <class T> void a3(void (*)(T[2]));
template <class X> concept CA3 = requires { a3<X>(nullptr); };
static_assert(!CA3<int AS(1)>);
static_assert(CA3<int>);
template <class T> using E = void (*)(T);
template <class X> concept CE = requires { typename E<X>; };
static_assert(CE<int AS(1)>);
