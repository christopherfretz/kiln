// A structure or union cast to its own type is a GNU extension (C17 6.5.4p2
// requires a scalar type): clang warns with -pedantic only, at the `(`, naming
// the type as written (`ext_typecheck_cast_nonscalar`).
//@ std: c17
//@ args: -pedantic
//@ expect: compile-ok
//@ strict-diags
//@ diag: 13:33 warning "C99 forbids casting nonscalar type 'struct S' to the same type"
//@ diag: 14:43 warning "C99 forbids casting nonscalar type 'struct S' to the same type"
//@ diag: 15:31 warning "C99 forbids casting nonscalar type 'union U' to the same type"
struct S { int a; };
union U { int i; };
struct S f(struct S s) { return (struct S)s; }
struct S g(volatile struct S *p) { return (struct S)*p; }
union U h(union U u) { return (union U)u; }
