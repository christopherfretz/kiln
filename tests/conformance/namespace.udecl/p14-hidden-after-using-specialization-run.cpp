// [namespace.udecl]/14, [temp.inst]/3: in a class template specialization
// a member function (template) declared AFTER the using-declarator hides
// the base member it corresponds to, as in a non-template class: the
// members are redone in declaration order and the hiding is checked when
// the member is redone (WP-3403). With a dependent parameter-type-list
// (`f(T)`) only the specializations whose arguments make it correspond
// hide. Each call picks the derived class's member, or the base's through
// the using-declarator; a dependent call in a member of the
// specialization too (spec/ast.md §8.8: its found list may name the
// hidden shadow, the call still picks the hider).
//@ expect: run
//@ stdout: 3 2 | 5 2 | 1 7 | 9 2 | 8 12
extern "C" int printf(const char *, ...);
struct B {
  int f() { return 1; }
  int f(int) { return 2; }
  template <class U> int g(U) { return 6; }
};
template <class T> struct F : B {
  using B::f;
  int f() { return 3; }
};
template <class T> struct G : B {
  using B::f;
  int f(T) { return 5; }
};
template <class T> struct H : B {
  using B::g;
  using B::f;
  template <class U> int g(U) { return 7; }
};
template <class T> struct K : B {
  using B::f;
  int f() const { return 4; }
  int f() { return 9; }
};
struct S {
  static int m(int) { return 11; }
  static int m(double) { return 12; }
};
template <class T> struct M : S {
  using S::m;
  static int m(T) { return 8; }
  int call(T t) { return m(t); }
  int other(double d) { return m(d); }
};
int main() {
  F<int> a;
  G<int> b;
  G<char> c;
  H<int> h;
  K<int> k;
  printf("%d %d | ", a.f(), a.f(0));
  printf("%d %d | ", b.f(0), c.f(0));
  printf("%d %d | ", c.f(), h.g(1));
  printf("%d %d | ", k.f(), k.f(1));
  M<int> mm;
  printf("%d %d\n", mm.call(1), mm.other(1.0));
  return 0;
}
