// [dcl.init.aggr]/5.2: an element with no initializer and no default
// member initializer is copy-initialized from an empty list; when that
// fails, clang adds "in implicit initialization of ..." for each enclosing
// omitted element, innermost first: a field at the field, an array element
// at the list's '}', and the trailing elements of a runtime-sized array
// new (also at the '}').
//@ std: c++20
//@ expect: compile-fail
//@ strict-diags
//@ diag: 31:9 error "no matching constructor for initialization of 'D'"
//@ diag: 29:12 note "candidate constructor not viable: requires 1 argument, but 0 were provided"
//@ diag: 29:8 note "candidate constructor (the implicit copy constructor) not viable: requires 1 argument, but 0 were provided"
//@ diag: 29:8 note "candidate constructor (the implicit move constructor) not viable: requires 1 argument, but 0 were provided"
//@ diag: 30:21 note "in implicit initialization of field 'd' with omitted initializer"
//@ diag: 32:18 error "no matching constructor for initialization of 'D'"
//@ diag: 29:12 note "candidate constructor not viable: requires 1 argument, but 0 were provided"
//@ diag: 29:8 note "candidate constructor (the implicit copy constructor) not viable: requires 1 argument, but 0 were provided"
//@ diag: 29:8 note "candidate constructor (the implicit move constructor) not viable: requires 1 argument, but 0 were provided"
//@ diag: 32:18 note "in implicit initialization of array element 1 with omitted initializer"
//@ diag: 33:34 error "no matching constructor for initialization of 'D'"
//@ diag: 29:12 note "candidate constructor not viable: requires 1 argument, but 0 were provided"
//@ diag: 29:8 note "candidate constructor (the implicit copy constructor) not viable: requires 1 argument, but 0 were provided"
//@ diag: 29:8 note "candidate constructor (the implicit move constructor) not viable: requires 1 argument, but 0 were provided"
//@ diag: 33:34 note "in implicit initialization of trailing array elements in runtime-sized array new"
//@ diag: 37:9 error "reference member of type 'int &' uninitialized"
//@ diag: 34:17 note "uninitialized reference member is here"
//@ diag: 35:14 note "in implicit initialization of field 'a' with omitted initializer"
//@ diag: 36:14 note "in implicit initialization of field 'c' with omitted initializer"
struct D { D(int); };
struct E { int i; D d; };
E e = {1};
D *p = new D[3]{1};
void f(int n) { D *q = new D[n]{1}; (void)q; }
struct A { int &r; };
struct C { A a; int i; };
struct F { C c; };
F ff = {};
