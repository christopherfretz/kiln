// [dcl.attr.grammar] The class-A error (an attribute clang rejects for
// every unit kiln compiles, spec/sema.md §4.5) is an error, not a
// warning: `-w` does not suppress it. Class B and C attributes
// (`no_sanitize`, `nocommon`, `transparent_union`, which clang ignores in
// C++) are accepted silently. WP-805; the attributes this test first used
// (`selectany`, `weak_import`, `address_space`, `swift_context`) are
// implemented since WP-809, WP-1745 and WP-1803.
//@ expect: compile-fail
//@ strict-diags
//@ args: -w
//@ diag: 13:35 error "matrix types extension is disabled. Pass -fenable-matrix to enable it"

typedef float m2x2 __attribute__((matrix_type(2, 2)));
[[clang::no_sanitize("address")]] int sanitized();
int nc __attribute__((nocommon));
union __attribute__((transparent_union)) U { int *ip; float *fp; };
