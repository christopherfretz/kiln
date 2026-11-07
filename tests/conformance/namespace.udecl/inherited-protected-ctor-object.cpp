// [namespace.udecl]/19, [class.protected]/1, [class.inhctor.init]: an
// inherited constructor has the access of the base constructor, and the
// object of that access is the derived class it constructs, so a member
// of the derived class may call an inherited protected constructor
// (through two levels of `using`, and in a class template whose base is
// a specialization). A class that is not the constructed one, or a
// non-member, still may not; for a constructor the note is clang's
// "protected constructor can only be used to construct a base class
// subobject". Reduced from Catch2 v3.5.0's clara BasicResult::ok().
// Verified against clang 19.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 35:16 error "calling a protected constructor of class 'B'"
//@ diag: 19:3 note "protected constructor can only be used to construct a base class subobject"
//@ diag: 37:3 error "calling a protected constructor of class 'B'"
//@ diag: 19:3 note "declared protected here"
class B {
protected:
  B(int) {}
};
struct V : B { protected: using B::B; };
struct R : V {
  using V::V;
  static R make() { return {1}; }
  static R make2() { return R(1); }
};
template <class T> struct W : B {};
template <> struct W<void> : B { protected: using B::B; };
template <class T = void> struct S : W<T> {
  using W<T>::W;
  static S make() { return {2}; }
};
struct Q : B {
  Q() : B(0) {}
  void f() { R r(3); }
};
R outside(4);
int main() { R::make(); R::make2(); S<>::make(); }
