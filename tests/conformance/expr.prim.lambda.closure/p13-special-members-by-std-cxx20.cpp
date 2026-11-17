// [expr.prim.lambda.closure]/13: a closure type with a lambda-capture has
// no default constructor and a deleted copy assignment operator; without
// one both are defaulted, from C++20 only (C++17 [expr.prim.lambda]/11:
// no default constructor and a deleted copy assignment operator for every
// closure type). The variable whose default initialization failed is
// still checked when it is assigned.
// C++20 sibling of p13-special-members-by-std.cpp.
// Found running GCC's C++ testsuite.
//@ expect: compile-fail
//@ strict-diags
//@ args: -std=c++20
//@ diag: 27:17 error "no matching constructor for initialization of 'decltype(lam)' (aka '(lambda at tests/conformance/expr.prim.lambda.closure/p13-special-members-by-std-cxx20.cpp:26:14)')"
//@ diag: 26:14 note "candidate constructor (the implicit copy constructor) not viable: no known conversion from 'int' to 'const (lambda at tests/conformance/expr.prim.lambda.closure/p13-special-members-by-std-cxx20.cpp:26:14)' for 1st argument"
//@ diag: 26:14 note "candidate constructor (the implicit move constructor) not viable: no known conversion from 'int' to '(lambda at tests/conformance/expr.prim.lambda.closure/p13-special-members-by-std-cxx20.cpp:26:14)' for 1st argument"
//@ diag: 28:17 error "no matching constructor for initialization of 'decltype(lam)' (aka '(lambda at tests/conformance/expr.prim.lambda.closure/p13-special-members-by-std-cxx20.cpp:26:14)')"
//@ diag: 26:14 note "candidate constructor (the implicit copy constructor) not viable: requires 1 argument, but 0 were provided"
//@ diag: 26:14 note "candidate constructor (the implicit move constructor) not viable: requires 1 argument, but 0 were provided"
//@ diag: 29:8 error "object of type 'decltype(lam)' (aka '(lambda at tests/conformance/expr.prim.lambda.closure/p13-special-members-by-std-cxx20.cpp:26:14)') cannot be assigned because its copy assignment operator is implicitly deleted"
//@ diag: 26:14 note "lambda expression begins here"
//@ diag: 34:5 error "object of type '(lambda at tests/conformance/expr.prim.lambda.closure/p13-special-members-by-std-cxx20.cpp:33:12)' cannot be assigned because its copy assignment operator is implicitly deleted"
//@ diag: 33:12 note "lambda expression begins here"
//@ diag: 39:8 error "object of type 'decltype(lam)' (aka '(lambda at tests/conformance/expr.prim.lambda.closure/p13-special-members-by-std-cxx20.cpp:37:14)') cannot be assigned because its copy assignment operator is implicitly deleted"
//@ diag: 41:15 note "in instantiation of function template specialization 'g<int>' requested here"
//@ diag: 37:14 note "lambda expression begins here"
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
