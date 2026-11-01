// [class.default.ctor], [dcl.init.general]/7: default-initializing a
// variable of a class with no usable default constructor is ill-formed;
// clang reports "no matching constructor for initialization" with the
// candidates for no arguments. Verified against clang 19 (WP-191).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 46:13 warning "parentheses were disambiguated as redundant parentheses around declaration of variable named 'x'"
//@ diag: 46:12 note "add enclosing parentheses to perform a function-style cast"
//@ diag: 46:13 note "remove parentheses to silence this warning"
//@ diag: 46:14 error "no matching constructor for initialization of 'S'"
//@ diag: 44:12 note "candidate constructor not viable: requires 1 argument, but 0 were provided"
//@ diag: 44:8 note "candidate constructor (the implicit copy constructor) not viable: requires 1 argument, but 0 were provided"
//@ diag: 44:8 note "candidate constructor (the implicit move constructor) not viable: requires 1 argument, but 0 were provided"
//@ diag: 46:20 error "no matching constructor for initialization of 'S'"
//@ diag: 44:12 note "candidate constructor not viable: requires 1 argument, but 0 were provided"
//@ diag: 44:8 note "candidate constructor (the implicit copy constructor) not viable: requires 1 argument, but 0 were provided"
//@ diag: 44:8 note "candidate constructor (the implicit move constructor) not viable: requires 1 argument, but 0 were provided"
//@ diag: 46:32 error "no matching constructor for initialization of 'S'"
//@ diag: 44:12 note "candidate constructor not viable: requires 1 argument, but 0 were provided"
//@ diag: 44:8 note "candidate constructor (the implicit copy constructor) not viable: requires 1 argument, but 0 were provided"
//@ diag: 44:8 note "candidate constructor (the implicit move constructor) not viable: requires 1 argument, but 0 were provided"
//@ diag: 47:3 error "no matching constructor for initialization of 'S'"
//@ diag: 44:12 note "candidate constructor not viable: requires 1 argument, but 0 were provided"
//@ diag: 44:8 note "candidate constructor (the implicit copy constructor) not viable: requires 1 argument, but 0 were provided"
//@ diag: 44:8 note "candidate constructor (the implicit move constructor) not viable: requires 1 argument, but 0 were provided"
//@ diag: 49:3 error "no matching constructor for initialization of 'A'"
//@ diag: 48:12 note "candidate constructor not viable: requires 1 argument, but 0 were provided"
//@ diag: 48:20 note "candidate constructor not viable: requires 1 argument, but 0 were provided"
//@ diag: 48:8 note "candidate constructor (the implicit copy constructor) not viable: requires 1 argument, but 0 were provided"
//@ diag: 48:8 note "candidate constructor (the implicit move constructor) not viable: requires 1 argument, but 0 were provided"
//@ diag: 51:5 error "call to deleted constructor of 'Del'"
//@ diag: 50:14 note "'Del' has been explicitly marked deleted here"
//@ diag: 53:4 error "calling a private constructor of class 'Pv'"
//@ diag: 52:22 note "declared private here"
//@ diag: 54:32 error "no matching constructor for initialization of 'S'"
//@ diag: 55:15 note "in instantiation of function template specialization 't<S>' requested here"
//@ diag: 44:12 note "candidate constructor not viable: requires 1 argument, but 0 were provided"
//@ diag: 44:8 note "candidate constructor (the implicit copy constructor) not viable: requires 1 argument, but 0 were provided"
//@ diag: 44:8 note "candidate constructor (the implicit move constructor) not viable: requires 1 argument, but 0 were provided"
//@ diag: 56:3 error "no matching constructor for initialization of 'S[2]'"
//@ diag: 44:12 note "candidate constructor not viable: requires 1 argument, but 0 were provided"
//@ diag: 44:8 note "candidate constructor (the implicit copy constructor) not viable: requires 1 argument, but 0 were provided"
//@ diag: 44:8 note "candidate constructor (the implicit move constructor) not viable: requires 1 argument, but 0 were provided"
struct S { S(int); };
int x;
void f() { S(x); S y; static S z; }
S g1;
struct A { A(int); A(double); };
A g2;
struct Del { Del() = delete; };
Del g3;
struct Pv { private: Pv(); };
Pv g4;
template<class T> void t() { T v; }
template void t<S>();
S arr[2];
