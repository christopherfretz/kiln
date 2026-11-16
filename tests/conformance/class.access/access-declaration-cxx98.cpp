// C++98 [class.access.dcl]: an access declaration (`A::foo;` in a
// member-specification) adjusts access as `using A::foo;` would; it is
// deprecated (clang 22 -Wdeprecated, at the unqualified-id). Found running
// GCC's C++ testsuite (WP-1466).
//@ expect: run
//@ std: c++98
//@ strict-diags
//@ diag: 15:35 warning "access declarations are deprecated; use using declarations instead"
//@ diag: 15:43 warning "access declarations are deprecated; use using declarations instead"
//@ diag: 15:49 warning "access declarations are deprecated; use using declarations instead"
//@ diag: 15:55 warning "access declarations are deprecated; use using declarations instead"
//@ diag: 15:62 warning "access declarations are deprecated; use using declarations instead"
//@ diag: 16:45 warning "access declarations are deprecated; use using declarations instead"
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
