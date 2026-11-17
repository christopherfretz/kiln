// GNU attributes before a namespace-scope asm-declaration are skipped
// silently, as clang 22 does (GCC 16.1 rejects them: "expected
// unqualified-id before 'asm'"); a C++11 attribute after them is ignored
// with a warning (CWG2262, [dcl.asm]/1; clang 22 rejects it). Found running
// GCC's C++ testsuite.
//@ expect: compile-ok
//@ requires: clang>=22
//@ strict-diags
//@ xfail(clang): clang 22 does not implement CWG2262 at namespace scope
//@ diag: 13:27 warning "'foo::bar' attribute ignored"
__attribute__((unused)) asm("");
__attribute__((foo)) asm("");
__attribute__((unused)) [[foo::bar]] asm("");
