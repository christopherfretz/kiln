// [dcl.init.general]/16.5, [expr.new]/23: the elements of `new T[n](args)`
// past the arguments are value-initialized, and the arguments may not
// exceed a constant bound. Both errors are reported at the allocated type,
// as clang does (found running GCC's C++ testsuite).
//@ std: c++20
//@ expect: compile-fail
//@ strict-diags
//@ diag: 19:14 error "no matching constructor for initialization of 'C'"
//@ diag: 17:12 note "candidate constructor not viable: requires 1 argument, but 0 were provided"
//@ diag: 17:8 note "candidate constructor (the implicit copy constructor) not viable: requires 1 argument, but 0 were provided"
//@ diag: 17:8 note "candidate constructor (the implicit move constructor) not viable: requires 1 argument, but 0 were provided"
//@ diag: 20:14 error "no matching constructor for initialization of 'C'"
//@ diag: 17:12 note "candidate constructor not viable: requires 1 argument, but 0 were provided"
//@ diag: 17:8 note "candidate constructor (the implicit copy constructor) not viable: requires 1 argument, but 0 were provided"
//@ diag: 17:8 note "candidate constructor (the implicit move constructor) not viable: requires 1 argument, but 0 were provided"
//@ diag: 21:16 error "excess elements in array initializer"
struct C { C(int); };
void f(int n) {
  C *a = new C[3](8, 9);
  C *b = new C[n](8, 9);
  int *c = new int[1](1, 2);
}
