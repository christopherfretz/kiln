// [class.union.anon], [class.pre]/1, [temp.mem]: the unnamed member
// classes of a class template (an anonymous struct and union, an unnamed
// class with a declarator) are instantiated with it, beside a named member
// class template; the negative twin is unnamed-class-template.cpp.
//@ expect: run
//@ std: c++20
//@ stdout: 7 9 3 5

#include <cstdio>

template <class T> struct A {
  struct { T a; };
  union { T b; long c; };
  struct { T d; } named;
  template <class U> struct M { U m; };
  M<T> e;
};

int main() {
  A<int> x;
  x.a = 7;
  x.b = 9;
  x.named.d = 3;
  x.e.m = 5;
  std::printf("%d %d %d %d\n", x.a, x.b, x.named.d, x.e.m);
}
