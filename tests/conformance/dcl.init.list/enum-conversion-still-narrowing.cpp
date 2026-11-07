// [dcl.init.list]/7.4: a conversion from an unscoped enumeration that is
// not its integral promotion still narrows when the target cannot hold
// every value of the enum's underlying type.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 13:25 error "non-constant-expression cannot be narrowed from type 'E1' to 'short' in initializer list"
//@ diag: 13:25 note "insert an explicit cast to silence this issue"
//@ diag: 14:21 error "non-constant-expression cannot be narrowed from type 'E3' to 'int' in initializer list"
//@ diag: 14:21 note "insert an explicit cast to silence this issue"
enum E1 { A = 0, B = 30000 };
enum E3 { P = 1, Q = 0x80000000u };

short s(E1 v) { short x{v}; return x; }
int t(E3 v) { int x{v}; return x; }
