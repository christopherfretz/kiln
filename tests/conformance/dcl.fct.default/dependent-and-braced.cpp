// [dcl.fct.default]/5, [temp.inst]/12: a default argument depending on a
// template parameter (braced or not) is instantiated with the function;
// braced default arguments of members are parsed after the class.
//@ expect: run
//@ exit: 0
template<int N> struct C { int v = N; };
struct P { int a = 3, b = 4; };
template<class T> int f(C<sizeof(T)> c = {}) { return c.v; }
template<class T> int g(int x = sizeof(T), int y = {sizeof(T) * 2}) { return x + y; }
template<class T> int h(T t = {}) { return int(t) + 1; }
struct S {
  int m(P p = {}, int q = {5}) { return p.a + p.b + q; }
  int n(P p = {1, 2}) { return p.a * 10 + p.b; }
  template<class U> int k(C<sizeof(U)> c = {}) { return c.v; }
};
template<class T> struct TS {
  int m(C<sizeof(T)> c = {}, T t = {}) { return c.v + int(t); }
};
int main() {
  S s;
  int r = f<int>() + f<char>();            // 4 + 1
  r += g<short>();                          // 2 + 4
  r += h<int>() + h<double>();              // 1 + 1
  r += s.m() + s.n();                       // 12 + 12
  r += s.k<long>();                         // 8
  r += TS<short>().m();                     // 2
  return r - 47;
}
