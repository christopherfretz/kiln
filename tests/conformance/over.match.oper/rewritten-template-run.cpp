// [over.match.oper]/3.4: rewritten and reversed operator template
// specializations are called with the operands in parameter order.
//@ expect: run
//@ stdout: 1 1 1 1 1 1 0
extern "C" int printf(const char*, ...);
struct A { int v; };
template <class T> bool operator==(const A& a, const T& t) { return a.v == t; }
struct C {
  int v;
  template <class T> int operator<=>(const T& t) const { return v - t; }
};
template <class T> struct S {
  T v;
  template <class U> bool operator==(const U& u) const { return v == u; }
};
int main() {
  A a{3};
  C c{5};
  S<int> s{7};
  printf("%d %d %d %d %d %d %d\n", a == 3, 3 == a, 4 != a, c < 6, 6 > c, 7 == s, 8 == s);
  return 0;
}
