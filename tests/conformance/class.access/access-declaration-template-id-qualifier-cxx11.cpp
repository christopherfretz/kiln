// C++11 removed access declarations ([namespace.udecl], [diff.cpp03]): one
// whose qualifier contains a template-id (`I<T>::B::foo;`) is the same
// error as any other, as clang 22 reports, not a parse error (found
// running GCC's C++ testsuite). The C++98 twin is
// access-declaration-template-id-qualifier-cxx98.cpp.
//@ expect: compile-fail
//@ std: c++11
//@ strict-diags
//@ diag: 15:12 error "ISO C++11 does not allow access declarations; use using declarations instead"
//@ diag: 16:9 error "ISO C++11 does not allow access declarations; use using declarations instead"
//@ diag: 20:45 error "ISO C++11 does not allow access declarations; use using declarations instead"
struct B { void foo(); int bar; };
template<class T> struct I : public B {};
template<class T> class D : private I<T> {
  I<T>::B::foo;
  I<T>::bar;
public:
  void g() { this->foo(); }
};
struct F : private I<int> { public: I<int>::foo; };
int main() { D<int> d; d.g(); F f; f.foo(); }
