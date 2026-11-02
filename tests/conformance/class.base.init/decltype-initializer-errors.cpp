// [class.base.init]/2 (C++11): a decltype-specifier mem-initializer-id
// must denote a class type (a reference or scalar type does not, also
// once a dependent one is instantiated); `decltype(auto)` is not a
// type here ([dcl.type.auto.deduct]).
//@ expect: compile-fail
//@ std: c++23
//@ strict-diags
//@ diag: 18:14 error "constructor initializer 'decltype(i)' (aka 'int') does not name a class"
//@ diag: 19:9 error "'decltype(auto)' not allowed here"
//@ diag: 22:19 error "constructor initializer 'decltype(t)' (aka 'const B &') does not name a class"
//@ diag: 24:6 note "in instantiation of member function 'E<B>::E' requested here"
struct B {
  int v;
  B(int x) : v(x) {}
};
int i;
struct D : B {
  D(int x) : decltype(i)(x), B(x) {}
  D() : decltype(auto)(1) {}
};
template <class T> struct E : T {
  E(const T& t) : decltype(t)(t) {}
};
E<B> e(B(5));
