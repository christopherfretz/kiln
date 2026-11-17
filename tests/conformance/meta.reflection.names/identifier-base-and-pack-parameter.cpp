// [meta.reflection.names]/1-2 (P2996R13): a direct base class
// relationship has an identifier when its base class does
// (has_identifier(type_of(r))), and it is that class's (not that of an
// alias naming it; a class template specialization has none); a function
// parameter declared by a function parameter pack has none, and the
// parameters after the pack keep their own names (`args` covers every
// parameter it declares, whatever the number). Found running GCC's C++
// testsuite.
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ expect: compile-ok
//@ strict-diags
#include <meta>
using namespace std::meta;
constexpr auto ctx = access_context::unchecked();

struct A {};
template <class T> struct TB {};
using Alias = A;
struct D : Alias, TB<int> {};
static_assert(has_identifier(bases_of(^^D, ctx)[0]));
static_assert(identifier_of(bases_of(^^D, ctx)[0]) == "A");
static_assert(!has_identifier(bases_of(^^D, ctx)[1]));

auto lam = [] {};
struct L : decltype(lam) {};
static_assert(!has_identifier(bases_of(^^L, ctx)[0]));

template <class... T> void f(int first, T... args, int last);
template <> void f<char, long>(int first, char a, long b, int last);
constexpr auto fs = ^^f<char, long>;
static_assert(parameters_of(fs).size() == 4);
static_assert(has_identifier(parameters_of(fs)[0]));
static_assert(identifier_of(parameters_of(fs)[0]) == "first");
static_assert(has_identifier(parameters_of(fs)[3]));
static_assert(identifier_of(parameters_of(fs)[3]) == "last");

template <class... T> void g(T... args, int tail) {}
template void g<>(int);
template void g<int, int, int>(int, int, int, int);
static_assert(identifier_of(parameters_of(^^g<>)[0]) == "tail");
static_assert(!has_identifier(parameters_of(^^g<int, int, int>)[0]));
static_assert(!has_identifier(parameters_of(^^g<int, int, int>)[2]));
static_assert(identifier_of(parameters_of(^^g<int, int, int>)[3]) == "tail");
