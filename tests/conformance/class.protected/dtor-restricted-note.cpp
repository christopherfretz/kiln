// [class.protected]/1, [class.dtor]/15 (WP-684): a protected destructor
// named by a derived class's member for an object that is not a base
// class subobject is inaccessible: a local, a static or thread_local
// variable, an array element, a temporary (a discarded call result, a
// thrown object, a by-value argument, a reference-bound temporary, a
// returned prvalue), `delete`, `delete[]` and
// `new B[2]`. clang's note says such a destructor "can only be used to
// destroy a base class subobject". Destroying the base subobject itself is
// fine. Verified against clang 19.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 50:25 error "calling a protected destructor of class 'B'"
//@ diag: 46:28 note "protected destructor can only be used to destroy a base class subobject"
//@ diag: 51:28 error "calling a protected destructor of class 'B'"
//@ diag: 46:28 note "protected destructor can only be used to destroy a base class subobject"
//@ diag: 52:23 error "variable of type 'B' has protected destructor"
//@ diag: 46:28 note "protected destructor can only be used to destroy a base class subobject"
//@ diag: 53:20 error "temporary of type 'B' has protected destructor"
//@ diag: 46:28 note "protected destructor can only be used to destroy a base class subobject"
//@ diag: 53:20 error "exception object of type 'B' has protected destructor"
//@ diag: 46:28 note "protected destructor can only be used to destroy a base class subobject"
//@ diag: 54:19 error "temporary of type 'B' has protected destructor"
//@ diag: 46:28 note "protected destructor can only be used to destroy a base class subobject"
//@ diag: 55:14 error "temporary of type 'B' has protected destructor"
//@ diag: 46:28 note "protected destructor can only be used to destroy a base class subobject"
//@ diag: 56:25 error "temporary of type 'B' has protected destructor"
//@ diag: 46:28 note "protected destructor can only be used to destroy a base class subobject"
//@ diag: 57:27 error "temporary of type 'B' has protected destructor"
//@ diag: 46:28 note "protected destructor can only be used to destroy a base class subobject"
//@ diag: 58:14 error "temporary of type 'B' has protected destructor"
//@ diag: 46:28 note "protected destructor can only be used to destroy a base class subobject"
//@ diag: 60:18 error "temporary of type 'B' has protected destructor"
//@ diag: 46:28 note "protected destructor can only be used to destroy a base class subobject"
//@ diag: 61:29 error "variable of type 'B' has protected destructor"
//@ diag: 46:28 note "protected destructor can only be used to destroy a base class subobject"
//@ diag: 62:16 error "variable of type 'B' has protected destructor"
//@ diag: 46:28 note "protected destructor can only be used to destroy a base class subobject"
//@ diag: 62:21 error "temporary of type 'B' has protected destructor"
//@ diag: 46:28 note "protected destructor can only be used to destroy a base class subobject"
//@ diag: 65:16 error "variable of type 'B' has protected destructor"
//@ diag: 68:17 note "in instantiation of member function 'DT<B>::f' requested here"
//@ diag: 46:28 note "protected destructor can only be used to destroy a base class subobject"
//@ diag: 66:25 error "calling a protected destructor of class 'B'"
//@ diag: 68:17 note "in instantiation of member function 'DT<B>::h' requested here"
//@ diag: 46:28 note "protected destructor can only be used to destroy a base class subobject"
struct B { B(); protected: ~B(); };
B make();
void take(B);
struct D : B {
  void a(B *p) { delete p; }
  void a2(B *p) { delete[] p; }
  void b() { static B s; }
  void c() { throw B(); }
  void d() { take(B{}); }
  void e() { make(); }
  void f() { B *p = new B[2]; (void)p; }
  void g() { const B &r = B(); (void)r; }
  void i() { B{}; }
  void j(B) {}
  B k() { return B(); }
  void l() { thread_local B t; }
  void m() { B x; B y[2]; }
};
template <class T> struct DT : T {
  void f() { T t; }
  void h(T *p) { delete p; }
};
template struct DT<B>;
struct F : B { F() : B() {} ~F() {} };
void v() { F f; }
