// [dcl.attr.grammar] The class-A error (an attribute clang implements
// that changes semantics, the ABI or layout, which kiln does not
// implement yet) is an error, not a warning: `-w` does not suppress it
// (spec/sema.md §4.5). Class B and C attributes (`no_sanitize`,
// `nocommon`, `transparent_union`, which clang ignores in C++) are
// accepted silently. WP-805.
// (WP-809 implemented the `selectany` this test first used.)
//@ skip(clang): kiln-only error for attributes kiln does not implement yet
//@ expect: compile-fail
//@ strict-diags
//@ args: -w
//@ diag: 14:20 error "'address_space' attribute is not supported by kiln yet"
//@ diag: 15:30 error "'weak_import' attribute is not supported by kiln yet"
int __attribute__((address_space(1))) *as;
extern int wi __attribute__((weak_import));
[[clang::no_sanitize("address")]] int sanitized();
int nc __attribute__((nocommon));
union __attribute__((transparent_union)) U { int *ip; float *fp; };
