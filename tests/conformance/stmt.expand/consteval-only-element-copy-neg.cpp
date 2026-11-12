// [stmt.expand], [expr.const.const]/5.1, [expr.const.imm]/2.1 (P4101R1):
// reading a member of an expansion variable that holds a non-null
// reflection into a non-constexpr local (`const char* n = e.name;`) is as
// ill-formed as passing it to a run-time call
// (consteval-only-element-neg.cpp): the glvalue `e` refers to an
// immediate object, so it is an immediate constant expression, and `n`'s
// initialization is not manifestly constant-evaluated. kiln rejects the
// plain constexpr variable (line 24) and the expansion variable (line 25)
// alike. GCC 16.1 accepts both (a GCC hole: it folds the initializer and
// checks no consteval-only leaf in it), so this test is not
// GCC-compared. Before WP-1330 kiln accepted lines 25 and 27.
//@ expect: compile-fail
//@ args: -ferror-limit=0 -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ strict-diags
//@ diag: 24:58 error "consteval-only value outside an immediate function context"
//@ diag: 25:74 error "consteval-only value outside an immediate function context"
//@ diag: 27:69 error "consteval-only value outside an immediate function context"
//@ diag: 28:34 note "in instantiation of function template specialization 't<int>' requested here"
#include <meta>
#include <cstdio>
struct E { std::meta::info r; const char* name; };
constexpr E plan[] = {{^^int, "int"}, {^^long, "long"}};
void h_copy() { constexpr E e = plan[0]; const char* n = e.name; std::puts(n); }
void g_copy() { template for (constexpr auto e : plan) { const char* n = e.name; std::puts(n); } }
template <class T>
void t() { template for (constexpr auto e : plan) { const char* n = e.name; std::puts(n); } }
int main() { h_copy(), g_copy(), t<int>(); }
