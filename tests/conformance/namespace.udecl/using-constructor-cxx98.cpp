// [namespace.udecl]/3 in C++11, [class.qual]/2: before C++11 a
// using-declaration or access declaration may not name a constructor
// (`using F::F;`, `F::F;`, `D<T>::D;`): one error, without the access
// declaration's deprecation warning, as clang 22 reports (found running
// GCC's C++ testsuite).
//@ expect: compile-fail
//@ std: c++98
//@ strict-diags
//@ diag: 15:25 error "using declaration cannot refer to a constructor"
//@ diag: 16:19 error "using declaration cannot refer to a constructor"
//@ diag: 16:25 warning "access declarations are deprecated; use using declarations instead"
//@ diag: 18:50 error "using declaration cannot refer to a constructor"
//@ diag: 19:44 error "using declaration cannot refer to a constructor"
struct F { F(); void f(); };
struct G : F { using F::F; };
struct H : F { F::F; F::f; };
template <class T> struct D { D(); };
template <class T> struct E : D<T> { using D<T>::D; };
template <class T> struct K : D<T> { D<T>::D; };
