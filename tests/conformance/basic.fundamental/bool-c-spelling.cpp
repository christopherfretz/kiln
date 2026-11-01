//@ strict-diags
//@ expect: compile-fail
//@ diag: 10:1 error "unknown type name '_Bool'"
//@ diag: 11:1 error "unknown type name '_Bool'"
//@ diag: 11:9 error "unknown type name '_Bool'"
//@ diag: 12:12 error "unknown type name '_Bool'"
//@ diag: 13:25 error "use of undeclared identifier '_Bool'"
// [lex.key]: `_Bool` is a C keyword only (clang KEYNOCXX); in C++ it is a
// plain identifier, so these uses name nothing.
_Bool b;
_Bool f(_Bool x);
struct S { _Bool m : 1; };
int g() { return sizeof(_Bool); }
