// [class.abstract]/2-5: a class with a pure virtual final overrider is abstract
// (a pure function overridden in a derived class is no longer pure there); an
// abstract class cannot be a variable, field, by-value parameter or return of a
// definition, array element, or new-expression type, while pointers, references
// and declarations are fine; a note names each pure function once; an abstract
// 'final' class warns (-Wabstract-final-class).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 35:3 error "variable type 'B' is an abstract class"
//@ diag: 24:15 note "unimplemented pure virtual method 'g' in 'B'"
//@ diag: 37:5 error "field type 'A' is an abstract class"
//@ diag: 23:16 note "unimplemented pure virtual method 'f' in 'A'"
//@ diag: 24:15 note "unimplemented pure virtual method 'g' in 'A'"
//@ diag: 43:12 error "parameter type 'B' is an abstract class"
//@ diag: 44:6 error "array of abstract class type 'A'"
//@ diag: 45:10 error "array of abstract class type 'B'"
//@ diag: 47:7 error "allocating an object of abstract class type 'B'"
//@ diag: 51:8 warning "abstract class is marked 'final'"
//@ diag: 52:16 note "unimplemented pure virtual method 'h' in 'F'"
//@ diag: 57:3 error "variable type 'G' is an abstract class"
//@ diag: 55:16 note "unimplemented pure virtual method 'k' in 'G'"
struct A {
  virtual void f() = 0;
  virtual int g() = 0;
};
struct B : A {
  void f() override;
};
struct C : B {
  int g() override;
};
A *pa;
A &ra = *pa;
C c;
B b;
struct S {
  A a;
  B *pb;
};
A make();
void take(A);
A make2();
void take2(B) {}
A arr[2];
B (*parr)[3];
void use() {
  new B;
  A *p = &c;
  (void)p;
}
struct F final {
  virtual void h() = 0;
};
struct G : C {
  virtual void k() = 0;
};
G g;
