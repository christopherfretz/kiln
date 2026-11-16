// C++98 [class.access.dcl], [depr.access.dcl]: an access declaration's
// qualifier may contain a template-id. Found running GCC's C++ testsuite.
//@ expect: compile-ok
//@ std: c++98
//@ diag: 15:12 warning "access declarations are deprecated; use using declarations instead"
//@ diag: 16:12 warning "access declarations are deprecated; use using declarations instead"
//@ diag: 21:45 warning "access declarations are deprecated; use using declarations instead"
//@ strict-diags

struct B { void foo(); int bar; };
template<class T> struct I : public B {};
template<class T> struct J { template<class U> struct K : B {}; };

template<class T> class D : private I<T> {
  I<T>::B::foo;
  I<T>::B::bar;
public:
  void g() { this->foo(); }
};
template<class T> struct E : J<T>::template K<T> {};
struct F : private I<int> { public: I<int>::foo; };

int main() { D<int> d; d.g(); F f; f.foo(); }
