// [dcl.fct]/13: a function template may be declared through a typedef or
// alias of function type, or through `decltype` of a function, with no
// function declarator; its specializations' signatures are those types,
// substituted (an alias template's dependent function type too). WP-1740.
//@ expect: run
//@ stdout: 3 2 8 7
#include <cstdio>

using FT = int(int);
int g(int);
template <class T> using Fn = T(T);

template <class T> FT tf;
template <class T> int tf(int x) { return x + int(sizeof(T)) + 1; }

template <class T> decltype(g) tf2;
template <class T> int tf2(int x) { return x; }

template <class T> Fn<T> tf3;
template <class T> T tf3(T x) { return x * 2; }

struct S {
  template <class T> static FT sm;
};
template <class T> int S::sm(int x) { return x + 3; }

int main() {
  int a = tf<char>(1);
  int b = tf2<long>(2);
  int c = tf3<int>(4);
  int d = S::sm<void>(4);
  std::printf("%d %d %d %d\n", a, b, c, d);
}
