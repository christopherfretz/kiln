// [temp.names]/3: after `typename`, a `<` following a splice-specifier
// opens its template argument list, and the splice-specialization-specifier
// is then the scope of a qualified name: `typename [: r :]<A>::type`, at
// namespace and block scope, with a reflection template parameter, and as
// a type in an expression. Found running GCC's C++ testsuite.
//@ expect: compile-ok
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ strict-diags
#include <meta>
template <int N> struct TCls { using type = int; static constexpr int n = N; };
template <class T> struct TT { using type = T; };
typename [:^^TCls:]<3>::type v1 = 1;
typename [:^^TT:]<long>::type v2 = 2;
static_assert(sizeof(v2) == sizeof(long));
template <std::meta::info R> constexpr int get() { typename [:R:]<4>::type x = 4; return x; }
static_assert(get<^^TCls>() == 4);
void f() { typename [:^^TCls:]<5>::type v3 = 5; (void)v3; }
static_assert(typename [:^^TCls:]<6>{}.n == 6);
template <class T> struct C { template <class U> struct N { using type = U; }; using type = T; };
typename [:^^C:]<int>::template N<long>::type v4 = 4;
static_assert(sizeof(v4) == sizeof(long));
constexpr auto r5 = ^^typename [:^^C:]<int>::type;
static_assert(std::meta::dealias(r5) == ^^int);
