//@ expect: compile-ok
//@ strict-diags
// [temp.spec.partial.match], [basic.type.qualifier]/3: an array type carries
// its element's cv-qualifiers, so `S<const F>` matches `const int[10]`
// with `F = int[10]` (libc++ `__copy_cv` behind common_reference of arrays).
template <class A, class B> constexpr bool same = false;
template <class A> constexpr bool same<A, A> = true;
template <class F> struct S { static constexpr int v = 0; };
template <class F> struct S<const F> { static constexpr int v = 1; using t = F; };
template <class F> struct S<volatile F> { static constexpr int v = 2; };
template <class F> struct S<const volatile F> { static constexpr int v = 3; };
using CA = int const[10];
using VA = int volatile[10];
using CVA = int const volatile[10];
static_assert(S<CA>::v == 1);
static_assert(same<S<CA>::t, int[10]>);
static_assert(S<VA>::v == 2);
static_assert(S<CVA>::v == 3);
template <class T> using AddC = const T;
static_assert(same<AddC<VA>, CVA>);
static_assert(same<const VA, CVA>);
