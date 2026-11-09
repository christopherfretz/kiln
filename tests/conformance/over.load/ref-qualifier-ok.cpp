// [over.load]/2.3: member functions that all have ref-qualifiers, or that
// differ in their parameter-type-lists, may be overloaded; a static member
// function and an explicit object member function take no part in the rule.
//@ std: c++23
//@ expect: compile-ok
struct S {
  int f() & { return 1; }
  int f() && { return 2; }
  int g() const & { return 3; }
  int g() && { return 4; }
  int h(int) & { return 5; }
  int h() { return 6; }
  static int s(int) { return 7; }
  int s() & { return 8; }
  int e(this S&) { return 9; }
  int e(int) & { return 10; }
  template<class T> int t(T) & { return 11; }
  template<class T> int t(T) && { return 12; }
};
// C++23 [basic.scope.scope]/3: object parameters with different
// cv-qualifiers do not correspond, so these are overloads.
struct C {
  int f() const & { return 1; }
  int f() { return 2; }
};
int use() {
  S s;
  C c;
  const C cc{};
  return s.f() + S().f() + s.g() + S().g() + s.h(0) + s.h() + S::s(0) +
         s.s() + s.e() + s.e(0) + s.t(0) + S().t(0) + c.f() + cc.f();
}
