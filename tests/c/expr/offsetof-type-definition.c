// C23 (N2350): defining a type in `__builtin_offsetof`'s
// type-id is a C23 extension (clang `ext_type_defined_in_offsetof`, at
// the tag); only the outermost tag is reported (WP-1312, WP-1306
// follow-up).
//@ std: c17
//@ args: -pedantic
//@ strict-diags
//@ diag: 12:35 warning "defining a type within '__builtin_offsetof' is a C23 extension"
//@ diag: 13:28 warning "defining a type within '__builtin_offsetof' is a C23 extension"
//@ diag: 14:34 warning "defining a type within '__builtin_offsetof' is a C23 extension"
//@ diag: 14:52 warning "defining a type within '__builtin_offsetof' is a C23 extension"
int a = __builtin_offsetof(struct S { int a; int b; }, b);
int b = __builtin_offsetof(struct { int a; int b; }, b);
int c = __builtin_offsetof(union U { int a; struct N { int z; } n; }, n);
int d = __builtin_offsetof(struct S, a);
