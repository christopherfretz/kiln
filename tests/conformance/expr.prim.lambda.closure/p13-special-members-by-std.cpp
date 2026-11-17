// [expr.prim.lambda.closure]/13: a closure type with a lambda-capture has
// no default constructor and a deleted copy assignment operator; without
// one both are defaulted, from C++20 only (C++17 [expr.prim.lambda]/11:
// no default constructor and a deleted copy assignment operator for every
// closure type). The variable whose default initialization failed is
// still checked when it is assigned.
// Found running GCC's C++ testsuite.
//@ expect: compile-fail
//@ strict-diags
//@ args: -std=c++17
//@ diag: 31:17 error "no matching constructor for initialization of 'decltype(lam)' (aka '(lambda at tests/conformance/expr.prim.lambda.closure/p13-special-members-by-std.cpp:30:14)')"
//@ diag: 30:14 note "candidate constructor (the implicit copy constructor) not viable: no known conversion from 'int' to 'const (lambda at tests/conformance/expr.prim.lambda.closure/p13-special-members-by-std.cpp:30:14)' for 1st argument"
//@ diag: 30:14 note "candidate constructor (the implicit move constructor) not viable: no known conversion from 'int' to '(lambda at tests/conformance/expr.prim.lambda.closure/p13-special-members-by-std.cpp:30:14)' for 1st argument"
//@ diag: 32:17 error "no matching constructor for initialization of 'decltype(lam)' (aka '(lambda at tests/conformance/expr.prim.lambda.closure/p13-special-members-by-std.cpp:30:14)')"
//@ diag: 30:14 note "candidate constructor (the implicit copy constructor) not viable: requires 1 argument, but 0 were provided"
//@ diag: 30:14 note "candidate constructor (the implicit move constructor) not viable: requires 1 argument, but 0 were provided"
//@ diag: 33:8 error "object of type 'decltype(lam)' (aka '(lambda at tests/conformance/expr.prim.lambda.closure/p13-special-members-by-std.cpp:30:14)') cannot be assigned because its copy assignment operator is implicitly deleted"
//@ diag: 30:14 note "lambda expression begins here"
//@ diag: 35:16 error "no matching constructor for initialization of 'decltype(nc)' (aka '(lambda at tests/conformance/expr.prim.lambda.closure/p13-special-members-by-std.cpp:34:13)')"
//@ diag: 34:13 note "candidate constructor (the implicit copy constructor) not viable: requires 1 argument, but 0 were provided"
//@ diag: 34:13 note "candidate constructor (the implicit move constructor) not viable: requires 1 argument, but 0 were provided"
//@ diag: 36:7 error "object of type 'decltype(nc)' (aka '(lambda at tests/conformance/expr.prim.lambda.closure/p13-special-members-by-std.cpp:34:13)') cannot be assigned because its copy assignment operator is implicitly deleted"
//@ diag: 34:13 note "lambda expression begins here"
//@ diag: 38:5 error "object of type '(lambda at tests/conformance/expr.prim.lambda.closure/p13-special-members-by-std.cpp:37:12)' cannot be assigned because its copy assignment operator is implicitly deleted"
//@ diag: 37:12 note "lambda expression begins here"
//@ diag: 43:8 error "object of type 'decltype(lam)' (aka '(lambda at tests/conformance/expr.prim.lambda.closure/p13-special-members-by-std.cpp:41:14)') cannot be assigned because its copy assignment operator is implicitly deleted"
//@ diag: 45:15 note "in instantiation of function template specialization 'g<int>' requested here"
//@ diag: 41:14 note "lambda expression begins here"
void f(int i) {
  auto lam = [i] {};
  decltype(lam) lam2 = {1};
  decltype(lam) lam3;
  lam3 = lam;
  auto nc = [] {};
  decltype(nc) nc2;
  nc2 = nc;
  auto r = [&i] {};
  r = r;
}
template <class T> void g(T i) {
  auto lam = [i] {};
  decltype(lam) lam3 = lam;
  lam3 = lam;
}
template void g(int);
