// C23 6.4.2.1: identifiers follow UAX #31 (XID_Start, XID_Continue) as
// in C++; clang also accepts the mathematical notation profile as an
// extension, and an emoji is in neither.
//@ std: c23
//@ args: -pedantic
//@ expect: compile-fail
//@ strict-diags
//@ diag: 12:6 error "character <U+1F600> not allowed in an identifier"
//@ diag: 13:5 error "unexpected character <U+00A8>"
//@ diag: 14:5 warning "mathematical notation character <U+2202> in an identifier is a Clang extension"
//@ diag: 14:9 warning "mathematical notation character <U+2070> in an identifier is a Clang extension"
int z😀;
int ¨y;
int ∂x\u2070;
int \u00E9t̀ = 1, *p = &ét\u0300, *s = &y;
