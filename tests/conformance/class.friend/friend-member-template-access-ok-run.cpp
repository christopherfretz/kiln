// [class.friend]/9, [temp.friend]/1: a friend declaration naming a member
// template (or a constructor or conversion function template) of another
// class must be able to name it. These can: the member is public, or the
// befriending class is itself a friend of the member's class, or the
// friend is declared in a class template (clang checks none of those,
// kiln follows it). Each befriended member is then usable by the friend.
// Found running GCC's C++ testsuite.
//@ expect: run
class A {
  template <class U> A(U, U) : v(1) {}
  template <class U> operator U *() { return nullptr; }
  template <class U> int f(U) { return 2; }
  friend class Y;
  template <class T> friend class X;

public:
  A() : v(0) {}
  template <class U> A(U, U, U) : v(3) {}
  template <class U> int pf(U) { return 4; }
  int v;
};
class Pub {
  template <class U> friend A::A(U, U, U);
  template <class U> friend int A::pf(U);
};
class Y {
  template <class U> friend A::A(U, U);
  template <class U> friend A::operator U *();
  template <class U> friend int A::f(U);

public:
  static int g() {
    A a(1, 2);
    int *p = a;
    return a.v + a.f(0) + (p == nullptr);
  }
};
template <class T> class X {
  template <class U> friend A::A(U, U);
  template <class U> friend int A::f(U);

public:
  static int g() { return A(T(), T()).v + A().f(T()); }
};
int main() { return Y::g() == 4 && X<int>::g() == 3 && A(1, 2, 3).v == 3 ? 0 : 1; }
