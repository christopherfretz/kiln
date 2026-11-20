// [namespace.udecl]/1, [temp.variadic]/5.5: a using-declarator followed
// by `...` is a pack expansion. At class scope it names a member of each
// base (`using Ts::f...;`, inheriting constructors `using Ts::Ts...;`),
// also when the pack belongs to the enclosing class template of a member
// class template, which is instantiated with the pack expanded. At block
// scope ([namespace.udecl]/8 allows only non-members there) it may name
// an enumerator of each element; an empty expansion declares nothing.
//@ expect: run
//@ std: c++26
//@ stdout: 1 2 2 4 5 7 11 1
#include <cstdio>
struct X { int f(int) { return 1; } X() {} X(int) {} };
struct Y { int f(double) { return 2; } Y() {} Y(int, int) {} };
template <class... Ts> struct B : Ts... {
  using Ts::f...;
  using Ts::Ts...;
};
template <class... Ts> struct O {
  template <class U> struct I : Ts... {
    using Ts::f...;
    int g(U u) { return f(u); }
  };
};
struct P { operator int() const { return 4; } };
struct Q { operator double() const { return 5.0; } };
template <class... Ts> struct Cv : Ts... { using Ts::operator typename Ts::type...; };
struct PP : P { using type = int; };
struct QQ : Q { using type = double; };
template <class T> struct E { enum { A = sizeof(T) * 7 }; };
template <class... Ts> int one() { using E<Ts>::A...; return A; }
template <class... Ts> int none() { using E<Ts>::A...; return 11; }
int main() {
  B<X, Y> b(1, 2);
  B<X, Y> c(1);
  O<X, Y>::I<double> i;
  Cv<PP, QQ> cv;
  int n = cv.operator int();
  double d = cv.operator double();
  std::printf("%d %d %d %d %d %d %d %d\n", b.f(1), c.f(1.0), i.g(2.0), n, int(d),
              one<char>(), none<>(), O<X>::I<int>().g(0));
}
