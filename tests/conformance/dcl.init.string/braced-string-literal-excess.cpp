// [dcl.init.string]/1, [dcl.init.aggr]/12: a character array initialized
// by a braced string literal takes no further initializer (clang's
// "excess elements in char array initializer"), and the literal must fit
// with its terminating null character ([dcl.init.string]/2). WP-668.
// Verified against clang 19.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 12:19 error "excess elements in char array initializer"
//@ diag: 13:12 error "initializer-string for char array is too long"
//@ diag: 15:6 error "initializer-string for char array is too long"

char a[] = {"ab", 'c'};
char b[2] {"ab"};
struct S { char m[3]; };
S s{{"abc"}};
