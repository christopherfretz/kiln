// [expr.new]/2 with [dcl.type.general]/3: a parenthesized new type-id may not
// define a class. A new-type-id cannot contain a class body at all: in
// `new struct S {}` the braces are the new-initializer, so it allocates
// the (incomplete) elaborated type, as clang reports.
//@ std: c++20
//@ expect: compile-fail
//@ strict-diags
//@ diag: 12:24 error "'S1' cannot be defined in a type specifier"
//@ diag: 13:16 error "allocation of incomplete type 'struct S2'"
//@ diag: 13:23 note "forward declaration of 'S2'"
//@ diag: 14:28 error "'S3' cannot be defined in a type specifier"
void *g1 = new (struct S1 {});
void *g2 = new struct S2 {};
void *g3 = new (const enum S3 { e }[2]);
