// [meta.reflection.queries] (P2996R13): has_parent is false for an entity
// with C language linkage, so parent_of of an `extern "C"` function (bare
// form) or of a variable declared in an `extern "C"` block throws;
// GCC 16.1 rejects both with the same std::meta::exception.
//@ expect: compile-fail
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ strict-diags
//@ diag: 17:15 error "static assertion expression is not an integral constant expression"
//@ diag: 17:15 note "reflection does not represent an entity with parent"
//@ diag: 18:15 error "static assertion expression is not an integral constant expression"
//@ diag: 18:15 note "reflection does not represent an entity with parent"
#include <meta>
extern "C" int cfn(int);
extern "C" { int cv; }
static_assert(std::meta::parent_of(^^cfn) == ^^::);
static_assert(std::meta::parent_of(^^cv) == ^^::);
