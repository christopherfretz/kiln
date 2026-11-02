// [temp.names]/3, [temp.arg.explicit]: clang 19 wording of
// pre-cxx20-compat-adl-only-template-id.cpp, reached through
// -Wpre-c++20-compat-pedantic (which includes -Wpre-c++20-compat).
//@ requires: clang<22
//@ expect: compile-fail
//@ std: c++20
//@ args: -Wpre-c++20-compat-pedantic
//@ strict-diags
//@ diag: 26:9 warning "use of function template name with no prior function template declaration in function call with explicit template arguments is incompatible with C++ standards before C++20"
//@ diag: 27:9 warning "use of function template name with no prior function template declaration in function call with explicit template arguments is incompatible with C++ standards before C++20"
//@ diag: 28:9 error "no matching function for call to 'q'"
//@ diag: 23:25 note "candidate function template not viable: no known conversion from 'M::S' to 'int' for 2nd argument"
//@ diag: 22:25 note "candidate function template not viable: requires 1 argument, but 2 were provided"
//@ diag: 28:9 warning "use of function template name with no prior function template declaration in function call with explicit template arguments is incompatible with C++ standards before C++20"
//@ diag: 29:9 error "use of undeclared identifier 'r'"
//@ diag: 30:39 warning "use of function template name with no prior function template declaration in function call with explicit template arguments is incompatible with C++ standards before C++20"
//@ diag: 32:37 warning "use of function template name with no prior function template declaration in function call with explicit template arguments is incompatible with C++ standards before C++20"
//@ diag: 30:39 warning "use of function template name with no prior function template declaration in function call with explicit template arguments is incompatible with C++ standards before C++20"
//@ diag: 31:9 note "in instantiation of function template specialization 'h<M::S>' requested here"
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
