// [dcl.init.general]/16.6.2.2: initializing an aggregate from a
// parenthesized expression list is C++20 (P0960); before it, a class with
// no viable constructor or an array is an error. Found running GCC's C++
// testsuite.
//@ std: c++17
//@ expect: compile-fail
//@ strict-diags
//@ diag: 22:3 error "no matching constructor for initialization of 'P'"
//@ diag: 21:8 note "candidate constructor (the implicit copy constructor) not viable: requires 1 argument, but 2 were provided"
//@ diag: 21:8 note "candidate constructor (the implicit move constructor) not viable: requires 1 argument, but 2 were provided"
//@ diag: 21:8 note "candidate constructor (the implicit default constructor) not viable: requires 0 arguments, but 2 were provided"
//@ diag: 23:3 error "no matching constructor for initialization of 'P'"
//@ diag: 21:8 note "candidate constructor (the implicit copy constructor) not viable: no known conversion from 'int' to 'const P' for 1st argument"
//@ diag: 21:8 note "candidate constructor (the implicit move constructor) not viable: no known conversion from 'int' to 'P' for 1st argument"
//@ diag: 21:8 note "candidate constructor (the implicit default constructor) not viable: requires 0 arguments, but 1 was provided"
//@ diag: 24:5 error "array initializer must be an initializer list"
//@ diag: 25:14 error "no matching constructor for initialization of 'P'"
//@ diag: 21:8 note "candidate constructor (the implicit copy constructor) not viable: requires 1 argument, but 2 were provided"
//@ diag: 21:8 note "candidate constructor (the implicit move constructor) not viable: requires 1 argument, but 2 were provided"
//@ diag: 21:8 note "candidate constructor (the implicit default constructor) not viable: requires 0 arguments, but 2 were provided"
struct P { int a, b; };
P p1(1, 2);
P p2(1);
int arr[2](1, 2);
void f() { P q(3, 4); (void)q; }
