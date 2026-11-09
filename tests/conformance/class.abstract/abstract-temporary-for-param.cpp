// [class.abstract]/3: no object of an abstract class can be created except
// as a base subobject: not a temporary bound to a reference from a braced
// list or by a converting constructor, an explicit type conversion, a
// by-value parameter of a deduced specialization, or a variable of a class
// template specialization found abstract once instantiated. Binding a
// derived object, pointers and references are fine. Found by the clang
// negative-test run (WP-912), fixed by WP-1017.
//@ std: c++20
//@ expect: compile-fail
//@ strict-diags
//@ diag: 38:8 error "allocating an object of abstract class type 'A'"
//@ diag: 24:16 note "unimplemented pure virtual method 'f' in 'A'"
//@ diag: 39:8 error "allocating an object of abstract class type 'A'"
//@ diag: 40:10 error "allocating an object of abstract class type 'const A'"
//@ diag: 41:3 error "allocating an object of abstract class type 'A'"
//@ diag: 42:3 error "allocating an object of abstract class type 'A'"
//@ diag: 43:3 error "allocating an object of abstract class type 'A'"
//@ diag: 44:12 error "allocating an object of abstract class type 'A'"
//@ diag: 51:8 error "variable type 'S<int>' is an abstract class"
//@ diag: 32:16 note "unimplemented pure virtual method 'f' in 'S'"
struct A {
  A() {}
  A(int) {}
  virtual void f() = 0;
};
struct D : A {
  D() {}
  D(int) {}
  void f() override {}
};
template <class> struct S {
  virtual void f() = 0;
};
void take(A &&);
void take_c(const A &);
template <class T> void by_value(T);
void g(A &a, D &d) {
  take({});
  take(0);
  take_c(1);
  A(0);
  (A)(0);
  static_cast<A>(d);
  by_value(a);
  take(D());
  take_c(d);
  by_value(&a);
  (void)static_cast<A &>(d);
  new D(1);
}
S<int> s;
S<char> *ps;
