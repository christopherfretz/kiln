// [stmt.expand], [expr.const]/25.3 (P2996R13): reading a member of a
// consteval-only expansion variable into a non-constexpr local (`const
// char* n = e.name;`) is as ill-formed as passing it to a run-time call
// (consteval-only-element-neg.cpp): the id-expression `e` is a
// potentially-evaluated expression of consteval-only type, and `n`'s
// initialization is not manifestly constant-evaluated. kiln rejects the
// plain constexpr variable (line 23) and the expansion variable (line 24)
// alike. GCC 16.1 accepts both (a GCC hole: it folds the initializer and
// checks no consteval-only leaf in it), so this test is not
// GCC-compared. Before WP-1330 kiln accepted lines 24 and 26.
//@ expect: compile-fail
//@ args: -ferror-limit=0 -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ strict-diags
//@ diag: 23:58 error "consteval-only expressions are only allowed in a constant-evaluated context"
//@ diag: 24:74 error "consteval-only expressions are only allowed in a constant-evaluated context"
//@ diag: 26:69 error "consteval-only expressions are only allowed in a constant-evaluated context"
//@ diag: 27:34 note "in instantiation of function template specialization 't<int>' requested here"
#include <meta>
#include <cstdio>
struct E { std::meta::info r; const char* name; };
constexpr E plan[] = {{^^int, "int"}, {^^long, "long"}};
void h_copy() { constexpr E e = plan[0]; const char* n = e.name; std::puts(n); }
void g_copy() { template for (constexpr auto e : plan) { const char* n = e.name; std::puts(n); } }
template <class T>
void t() { template for (constexpr auto e : plan) { const char* n = e.name; std::puts(n); } }
int main() { h_copy(), g_copy(), t<int>(); }
