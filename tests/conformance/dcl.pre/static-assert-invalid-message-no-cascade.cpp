// [dcl.pre]/10-12 (P2741): a static_assert message that is an invalid
// expression (an undeclared name, a missing member, no viable function, no
// literal operator for a ud-suffixed string) makes the declaration invalid:
// only the message's own error is reported, never "static assertion failed"
// (clang drops the declaration when the message has errors).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 13:22 error "use of undeclared identifier 'undeclared_thing'"
//@ diag: 15:26 error "no member named 'bad' in 'S'"
//@ diag: 17:22 error "no matching function for call to 'f'"
//@ diag: 16:5 note "candidate function not viable: no known conversion from 'const char[2]' to 'int' for 1st argument"
//@ diag: 18:26 error "no matching literal operator for call to 'operator\"\"_x'"
static_assert(false, undeclared_thing);
struct S { };
static_assert(false, S{}.bad);
int f(int);
static_assert(false, f("x"));
static_assert(false, "ab"_x);
