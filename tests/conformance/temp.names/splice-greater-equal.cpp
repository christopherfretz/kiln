// [temp.names]/4: in a template-argument-list only a `>` (or the first
// `>` of `>>`) ends the list; `>=` is one token, the relational operator,
// after a splice-expression as after any other operand, while `>` needs
// parentheses. Found running GCC's C++ testsuite.
//@ args: -freflection -I tests/support
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ expect: compile-ok
//@ strict-diags
constexpr int v = 42;
constexpr auto r = ^^v;
template <bool> struct S;
template <> struct S<true> {};
template <int N> struct I { static constexpr int n = N; };
S<[:r:] >= 41> s1;
S<[:r:] <= 43> s2;
S<([:r:] > 41)> s3;
S<[:r:] == 42> s4;
S<43 >= [:r:]> s5;
S<!([:r:] >= 43)> s6;
static_assert(I<([:r:] >= 42) + 1>::n == 2);
template <auto R> constexpr bool ge = (S<[:R:] >= 41>{}, true);
static_assert(ge<r>);
