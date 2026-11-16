// C++11 removed access declarations ([namespace.udecl], [diff.cpp03]):
// `A::foo;` in a member-specification is ill-formed, reported at the
// unqualified-id as clang 22 does (GCC 16.1 only warns), and recovered as
// the using-declaration it means, so nothing else is reported. Found
// running GCC's C++ testsuite (WP-1466).
//@ expect: compile-fail
//@ std: c++11
//@ strict-diags
//@ diag: 16:35 error "ISO C++11 does not allow access declarations; use using declarations instead"
//@ diag: 16:43 error "ISO C++11 does not allow access declarations; use using declarations instead"
//@ diag: 16:49 error "ISO C++11 does not allow access declarations; use using declarations instead"
//@ diag: 16:55 error "ISO C++11 does not allow access declarations; use using declarations instead"
//@ diag: 16:62 error "ISO C++11 does not allow access declarations; use using declarations instead"
//@ diag: 17:45 error "ISO C++11 does not allow access declarations; use using declarations instead"
struct A { int foo; void f() {} enum { e1 }; int operator()() { return 0; } protected: int g; };
struct B : private A { public: A::foo; A::f; A::g; A::e1; A::operator(); };
template<class T> struct C : T { public: T::foo; };
int main() {
  B b;
  b.foo = 1;
  b.f();
  b.g = 0;
  C<A> c;
  c.foo = 0;
  return b.g + c.foo + B::e1 + b();
}
