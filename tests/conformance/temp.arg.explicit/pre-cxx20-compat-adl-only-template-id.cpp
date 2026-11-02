// [temp.names]/3, [temp.arg.explicit] (P0846): from C++20 a call
// `q<int>(x)` whose `q` only argument-dependent lookup finds is valid;
// -Wpre-c++20-compat (off by default) flags it as incompatible with
// earlier standards at the name, at the same points as the C++17
// extension warning (temp.names/adl-only-template-id-cxx17.cpp): after
// the call's own diagnostics, and per instantiation for a dependent call.
// Clang 19 says "no prior function template declaration"
// (pre-cxx20-compat-adl-only-template-id-clang19.cpp).
//@ requires: clang>=22
//@ expect: compile-fail
//@ std: c++20
//@ args: -Wpre-c++20-compat
//@ strict-diags
//@ diag: 31:9 warning "use of function template name with no prior declaration in function call with explicit template arguments is incompatible with C++ standards before C++20"
//@ diag: 32:9 warning "use of function template name with no prior declaration in function call with explicit template arguments is incompatible with C++ standards before C++20"
//@ diag: 33:9 error "no matching function for call to 'q'"
//@ diag: 28:25 note "candidate function template not viable: no known conversion from 'M::S' to 'int' for 2nd argument"
//@ diag: 27:25 note "candidate function template not viable: requires 1 argument, but 2 were provided"
//@ diag: 33:9 warning "use of function template name with no prior declaration in function call with explicit template arguments is incompatible with C++ standards before C++20"
//@ diag: 34:9 error "use of undeclared identifier 'r'"
//@ diag: 35:39 warning "use of function template name with no prior declaration in function call with explicit template arguments is incompatible with C++ standards before C++20"
//@ diag: 37:37 warning "use of function template name with no prior declaration in function call with explicit template arguments is incompatible with C++ standards before C++20"
//@ diag: 35:39 warning "use of function template name with no prior declaration in function call with explicit template arguments is incompatible with C++ standards before C++20"
//@ diag: 36:9 note "in instantiation of function template specialization 'h<M::S>' requested here"
namespace M {
  struct S {};
  template<class T> int q(S);
  template<class T> int q(S, int);
  int r(S);
}
int a = q<int>(M::S{});
int b = q<long>(M::S{}, 1);
int c = q<int>(M::S{}, M::S{});
int d = r<int>(M::S{});
template<class U> int h(U u) { return q<int>(u); }
int e = h(M::S{});
template<class U> int h2() { return q<int>(M::S{}); }
int f = h2<int>();
