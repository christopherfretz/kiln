// [dcl.asm]/1 (CWG2262): an attribute-specifier-seq before an
// asm-declaration at namespace scope is allowed; no attribute appertains
// to it, so each is "'x' attribute ignored" [-Wignored-attributes], as at
// block scope. clang 22 rejects any attribute here ("an attribute list
// cannot appear here"); GCC 16.1 accepts and warns. The text decides.
// Found running GCC's C++ testsuite.
//@ expect: compile-ok
//@ oracle: gcc
//@ xfail(clang): clang 22 does not implement CWG2262 at namespace scope
//@ strict-diags
//@ diag: 17:3 warning "'maybe_unused' attribute ignored"
//@ diag: 18:3 warning "'foo::bar' attribute ignored"
//@ diag: 18:16 warning "'bar::baz' attribute ignored"
//@ diag: 19:1 warning "'alignas' attribute ignored"
//@ diag: 20:17 warning "'deprecated' attribute ignored"
[[]] asm("");
[[maybe_unused]] asm("");
[[foo::bar]] [[bar::baz(1)]] asm("");
alignas(8) asm("");
namespace N { [[deprecated]] asm(""); }
asm("");
