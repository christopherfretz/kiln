// [expr.reflect]/1-2: in `^^ nested-name-specifier template identifier`
// the identifier shall denote a template; a class, a data member, a
// namespace-scope variable and a non-template member function do not
// (clang's error for `template` before a non-template). GCC 16.1 accepts
// all four; the text decides. Found running GCC's C++ testsuite.
//@ expect: compile-fail
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ strict-diags
//@ diag: 17:35 error "'C' following the 'template' keyword does not refer to a template"
//@ diag: 18:35 error "'x' following the 'template' keyword does not refer to a template"
//@ diag: 19:35 error "'y' following the 'template' keyword does not refer to a template"
//@ diag: 20:35 error "'g' following the 'template' keyword does not refer to a template"
#include <meta>
struct S { struct C {}; int x; void g(); };
namespace N { int y; }
constexpr auto r1 = ^^S::template C;
constexpr auto r2 = ^^S::template x;
constexpr auto r3 = ^^N::template y;
constexpr auto r4 = ^^S::template g;
