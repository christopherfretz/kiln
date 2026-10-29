// [dcl.fct.def.default]/1-2: only special members (and comparisons) may be
// defaulted, with the type of the implicit declaration; /5: a defaulted function
// that would be deleted is deleted on its first declaration (with a warning) and
// an error out of line.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 25:14 error "only special member functions and comparison operators may be defaulted"
//@ diag: 26:12 error "only special member functions and comparison operators may be defaulted"
//@ diag: 32:12 error "only special member functions and comparison operators may be defaulted"
//@ diag: 34:23 error "only special member functions and comparison operators may be defaulted"
//@ diag: 35:6 error "the parameter for an explicitly-defaulted copy assignment operator must be an lvalue reference type"
//@ diag: 36:6 warning "explicitly defaulted copy assignment operator is implicitly deleted"
//@ diag: 36:6 note "function is implicitly deleted because its declared type does not match the type of an implicit copy assignment operator"
//@ diag: 37:7 error "explicitly-defaulted move assignment operator must return 'B &'"
//@ diag: 40:3 warning "explicitly defaulted default constructor is implicitly deleted"
//@ diag: 42:8 note "default constructor of 'C' is implicitly deleted because field 'r' of reference type 'int &' would not be initialized"
//@ diag: 40:9 note "replace 'default' with 'delete'"
//@ diag: 49:4 error "defaulting this default constructor would delete it after its first declaration"
//@ diag: 47:8 note "default constructor of 'D' is implicitly deleted because field 'r' of reference type 'int &' would not be initialized"
//@ diag: 54:3 warning "explicitly defaulted copy constructor is implicitly deleted"
//@ diag: 53:6 note "copy constructor of 'I' is implicitly deleted because field 'n' has a deleted copy constructor"
//@ diag: 51:13 note "'NC' has been explicitly marked deleted here"
//@ diag: 54:18 note "replace 'default' with 'delete'"
struct A {
  void f() = default;
  A(int) = default;
  A() = default;
  A(const A &) = default;
  A &operator=(const A &) = default;
  ~A() = default;
};
void g() = default;
struct B {
  B(const B &, int) = default;
  B &operator=(B) = default;
  B &operator=(const B &) volatile = default;
  int operator=(B &&) = default;
};
struct C {
  C() = default;
  C(const C &);
  int &r;
};
C::C(const C &) = default;
struct D {
  D();
  int &r;
};
D::D() = default;
struct V { volatile int v; V(const V &) = default; };
struct NC { NC(const NC &) = delete; };
struct I {
  NC n;
  I(const I &) = default;
  I &operator=(const I &) = default;
};
