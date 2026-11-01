//@ strict-diags
//@ expect: compile-fail
//@ diag: 32:22 error "no viable conversion from returned value of type 'int' to function return type 'S'"
//@ diag: 28:8 note "candidate constructor (the implicit copy constructor) not viable: no known conversion from 'int' to 'const S &' for 1st argument"
//@ diag: 28:8 note "candidate constructor (the implicit move constructor) not viable: no known conversion from 'int' to 'S &&' for 1st argument"
//@ diag: 33:22 error "no viable conversion from returned value of type 'int' to function return type 'E'"
//@ diag: 29:8 note "candidate constructor (the implicit copy constructor) not viable: no known conversion from 'int' to 'const E &' for 1st argument"
//@ diag: 29:8 note "candidate constructor (the implicit move constructor) not viable: no known conversion from 'int' to 'E &&' for 1st argument"
//@ diag: 29:21 note "explicit constructor is not a candidate"
//@ diag: 34:22 error "no viable conversion from returned value of type 'int' to function return type 'A'"
//@ diag: 30:8 note "candidate constructor (the implicit copy constructor) not viable: no known conversion from 'int' to 'const A &' for 1st argument"
//@ diag: 30:8 note "candidate constructor (the implicit move constructor) not viable: no known conversion from 'int' to 'A &&' for 1st argument"
//@ diag: 35:21 error "no viable conversion from returned value of type 'T' to function return type 'S'"
//@ diag: 28:8 note "candidate constructor (the implicit copy constructor) not viable: no known conversion from 'T' to 'const S &' for 1st argument"
//@ diag: 28:8 note "candidate constructor (the implicit move constructor) not viable: no known conversion from 'T' to 'S &&' for 1st argument"
//@ diag: 36:29 error "no viable conversion from returned value of type '_Atomic(S)' to function return type 'S'"
//@ diag: 28:8 note "candidate constructor (the implicit copy constructor) not viable: no known conversion from '_Atomic(S)' to 'const S &' for 1st argument"
//@ diag: 28:8 note "candidate constructor (the implicit move constructor) not viable: no known conversion from '_Atomic(S)' to 'S &&' for 1st argument"
//@ diag: 37:30 error "no viable conversion from returned value of type '_Atomic(S)' to function return type 'S'"
//@ diag: 28:8 note "candidate constructor (the implicit copy constructor) not viable: no known conversion from '_Atomic(S)' to 'const S &' for 1st argument"
//@ diag: 28:8 note "candidate constructor (the implicit move constructor) not viable: no known conversion from '_Atomic(S)' to 'S &&' for 1st argument"
//@ diag: 38:39 error "cannot initialize return object of type '_Atomic(S)' with an lvalue of type '_Atomic(S)'"
//@ diag: 41:17 error "no matching function for call to 'take'"
//@ diag: 40:6 note "candidate function not viable: no known conversion from 'S' to '_Atomic(S)' for 1st argument"
// [stmt.return]/2, [dcl.init.general]/16.6.3: a return operand that no
// constructor converts is clang's "no viable conversion from returned
// value ...", whatever the operand's type (WP-183).
struct S { int a, b; };
struct E { explicit E(int); };
struct A { A(int, int); };
struct T {};
S h1(int i) { return i; }
E h2(int i) { return i; }
A h3(int i) { return i; }
S h4(T &t) { return t; }
S h5(_Atomic(S) s) { return s; }
S h6(_Atomic(S) &s) { return s; }
_Atomic(S) h7(_Atomic(S) &s) { return s; }
// An argument is not converted to an _Atomic(T) class parameter.
void take(_Atomic(S));
void h8(S &s) { take(s); }
