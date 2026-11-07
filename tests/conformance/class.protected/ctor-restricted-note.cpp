// [class.protected]/1: a protected constructor named by a derived class
// for an object that is not of its own type is inaccessible; clang's note
// says such a constructor "can only be used to construct a base class
// subobject" rather than naming the required object type.
// Verified against clang 19.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 20:7 error "calling a protected constructor of class 'B'"
//@ diag: 13:3 note "protected constructor can only be used to construct a base class subobject"
class B {
  B(int, int) {}
protected:
  B(int) {}
public:
  ~B() {}
};
struct Q : B {
  Q() : B(0) {}
  void f() {
    B b(3);
  }
};
