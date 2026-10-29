// [temp.inst]/3, [class.base.init]: the ctor-initializer of a constructor of
// a class template is instantiated with the constructor; its mem-initializers
// are resolved and checked in the specialization (clang's
// InstantiateMemInitializers), including -Wreorder-ctor and -Wuninitialized.
// [temp.inst]/2: initializing an object of a specialization by a constructor
// instantiates the class first. [stmt.return]/2: a braced list with a
// type-dependent element is checked at instantiation.
//@ expect: compile-fail
//@ strict-diags
//@ args: -Wreorder-ctor -Wuninitialized
//@ diag: 19:12 warning "field 'b' will be initialized after field 'a'"
//@ diag: 31:10 note "in instantiation of member function 'S<int>::S"
//@ diag: 19:20 warning "field 'b' is uninitialized when used here"
//@ diag: 20:17 error "of type 'int' with an lvalue of type 'int *'"
//@ diag: 32:12 note "in instantiation of member function 'S<int *>::S"
struct A { A(int, int); };
template <class T> struct S {
  T a; T b; int c;
  S(T t) : b(t), a(b), c(1) {}
  S(T t, int) : c(t) {}
};
template <class T> struct R { A good(T t) { return {t, t}; } };
template <class T> struct C { T v; C(T x) : v(x) {} };
void take(C<long>);
void f() {
  C<int> c(1);
  C<short> d = 2;
  take(3);
  (void)C<char>{'x'};
  (void)R<int>().good(1);
  S<int> s(1);
  S<int *> p(nullptr, 0);
}
