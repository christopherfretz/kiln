// [class.friend]: a friend function or class may use the private members of
// the befriending class; a friend defined in the class (a hidden friend) is
// found only by argument-dependent lookup.
//@ expect: run
//@ stdout: 3 4 5 6
//@ xfail(kiln): needs ctor-initializers and constructor calls (WP-067b) and WP-068a ADL (hidden friend call)
extern "C" int printf(const char*, ...);
class A {
  int v;
  friend int peek(const A &a);
  friend class Spy;
  friend int twice(A a) { return a.v * 2 - 2; }
public:
  explicit A(int x) : v(x) {}
};
int peek(const A &a) { return a.v; }
class Spy {
public:
  static int look(const A &a) { return a.v + 1; }
};
int main() {
  A a(3);
  printf("%d %d %d %d\n", peek(a), twice(a), Spy::look(a) + 1, Spy::look(A(5)));
}
