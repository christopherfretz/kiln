// [class.mem.general]/5: a nested class or member class template can be
// declared and later defined in the member-specification; a class of the
// same name declared in a nested class, a local class, or a class first
// declared by a friend declaration is not a redeclaration of a member.
//@ std: c++20
//@ expect: compile-ok
//@ strict-diags
struct A {
  struct B;
  struct B { int v = 1; };
  template <class T> struct X;
  template <class T> struct X { T t{}; };
  enum E : int;
  enum E : int { e = 2 };
  struct Y;
  struct I { struct Y; };
  int f() {
    struct L;
    struct L { int w = 3; };
    return L().w;
  }
};
struct Q {
  friend struct R;
  struct R;
};
struct Q::R { int z = 4; };
struct A::Y {};
int use() { return A::B().v + A::X<int>().t + A::e + A().f() + Q::R().z; }
