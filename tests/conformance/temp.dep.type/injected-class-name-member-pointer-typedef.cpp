// [temp.dep.type]/1, [temp.local]/1, [dcl.mptr]/1, [expr.mptr.oper]/4: a
// member-pointer typedef `typedef int (Scan::*Fn)(int)` inside a class
// template names the current instantiation `Scan<C>`; after instantiation
// it is a pointer to member of the specialization, usable as the type of
// a static data member, of a function parameter and as the right operand
// of `->*` / `.*` with an object of the specialization (WP-608).
//@ expect: run
//@ strict-diags
//@ stdout: 11 22 13 4
#include <cstdio>

template <class C> struct Scan {
  typedef int (Scan::*Fn)(int);
  typedef int Scan::*Field;
  static Fn table[2];
  static Field field;
  int v = 4;
  int add(int a) { return a + 10; }
  int twice(int a) { return a * 2; }
  int apply(Fn f, int a) { return (this->*f)(a); }
  static int with(Scan &s, Fn f, int a) { return (s.*f)(a); }
  int read(Field f) const { return this->*f; }
  static_assert(__is_same(Fn, int (Scan<C>::*)(int)));
};

template <class C> typename Scan<C>::Fn Scan<C>::table[2] = {&Scan::add, &Scan::twice};
template <class C> typename Scan<C>::Field Scan<C>::field = &Scan::v;

int main() {
  Scan<char> s;
  static_assert(__is_same(Scan<char>::Fn, int (Scan<char>::*)(int)));
  int a = s.apply(Scan<char>::table[0], 1);
  int b = Scan<char>::with(s, Scan<char>::table[1], 11);
  int c = s.apply(&Scan<char>::add, 3);
  int d = s.read(Scan<char>::field);
  std::printf("%d %d %d %d\n", a, b, c, d);
  if (a != 11)
    return 1;
  if (b != 22)
    return 2;
  if (c != 13)
    return 3;
  return d == 4 ? 0 : 4;
}
