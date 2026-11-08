// [dcl.attr.grammar] An attribute clang implements that changes
// semantics, the ABI or layout (class A) and kiln does not implement is
// an error, also in a system header (the lines after the line marker
// with flag 3, [cpp.line] GNU extension), where warnings are suppressed:
// kiln never accepts a construct whose meaning it would change
// (spec/sema.md §4.5). clang implements all three. WP-805.
//@ skip(clang): kiln-only error for attributes kiln does not implement yet
//@ expect: compile-fail
//@ strict-diags
//@ diag: 12:30 error "'weak_import' attribute is not supported by kiln yet"
//@ diag: 13:30 error "'clang::musttail' attribute is not supported by kiln yet"
//@ diag: 14:23 error "'ms_struct' attribute is not supported by kiln yet"
# 12 "tests/conformance/dcl.attr.unknown/class-a-error-in-system-header.cpp" 3
extern int wi __attribute__((weak_import));
int g(int); int f(int x) { [[clang::musttail]] return g(x); }
struct __attribute__((ms_struct)) S { char c : 4; int i : 4; };
