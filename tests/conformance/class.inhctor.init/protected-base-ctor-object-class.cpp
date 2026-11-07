// [namespace.udecl]/19, [class.protected]/1: a constructor inherited from a
// protected base constructor is accessible where that constructor would be
// accessible to construct an object of the base class; the object is of the
// class the using-declarator is in (clang: the using-shadow's class), so its
// own members may call it — also through a chain of using-declarators.
// Elsewhere, including in a sibling derived class, it is not ([class.access]/4).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 33:23 error "calling a protected constructor of class 'A'"
//@ diag: 15:3 note "protected constructor can only be used to construct a base class subobject"
//@ diag: 35:16 error "calling a protected constructor of class 'A'"
//@ diag: 15:3 note "declared protected here"
class A {
protected:
  A(int v) : m(v) {}
  int m;
};
class B : public A {
protected:
  using A::A;
};
class C : public B {
public:
  static C ok() { return {1}; }
  static C ok2() { return C(2); }
  int get() const { return m; }

protected:
  using B::B;
};
struct D : A {
  D() : A(0) {}
  static void f() { C c(3); }
};
C g() { return C(4); }
int main() { return C::ok().get() + C::ok2().get() - 3; }
