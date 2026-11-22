// [dcl.attr.grammar]/1: an attribute-list element that is not an
// attribute-token ends the list at "expected ']'" after the previous
// token, and the specifier is skipped through its `]]`; a missing name
// after `scope::` is "expected identifier" (clang
// `ParseCXX11AttributeSpecifierInternal`). "unknown attribute ignored"
// is issued when the declaration's attributes are processed, so after
// the syntax errors later in the attribute-specifier-seq (the order is
// checked in crates/kiln_driver/tests/driver.rs). WP-1854. Verified
// against clang 22 and 19.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 22:11 error "expected ']'"
//@ diag: 22:3 warning "unknown attribute 'qux' ignored"
//@ diag: 23:21 error "expected ']'"
//@ diag: 23:3 warning "unknown attribute 'qux2' ignored"
//@ diag: 23:12 warning "unknown attribute 'quux' ignored"
//@ diag: 24:3 error "expected ']'"
//@ diag: 25:13 error "expected ']'"
//@ diag: 26:8 error "expected identifier"
//@ diag: 27:7 error "expected ']'"
//@ diag: 27:3 warning "unknown attribute 'foo' ignored"
[[qux(1, ][)]] int x;
[[qux2]] [[quux(1, ][)]] int y;
[[3]] int a;
[[nodiscard, 3]] int f();
[[foo::3]] int c;
[[foo, 3 4]] int d;
int e = x + y + a + c + d + f();
