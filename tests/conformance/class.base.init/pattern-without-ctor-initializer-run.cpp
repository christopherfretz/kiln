// [class.base.init]/9: a constructor of a class template, a constructor
// template and a member class's constructor written without a
// ctor-initializer (`S() {}`) still initialize every base and member
// implicitly in each instantiation: a member with a default member
// initializer from it, a base by its default constructor, a member of
// class type by its default constructor; beside a constructor of the
// same class whose ctor-initializer is written. The pattern's empty list
// is a record on the Ast since WP-3200 (spec/ast.md §8.6); instantiation
// is unchanged. Matches clang 19 and 22.
//@ expect: run
//@ stdout: 7 1 2 | 9 1 2 | 7 1 2 | 5 3 | 4 1 2 | 7 1 2
#include <cstdio>
struct B {
  int b;
  B() : b(1) {}
};
struct M {
  int m;
  M() : m(2) {}
};
template <class T> struct S : B {
  T v = 7;
  M w;
  S() {}
  S(int) : v(9) {}
  template <class U> S(U, U) {}
};
template <class T> struct O {
  struct I {
    T a = 5;
    T c = 3;
    I() {}
  };
};
template <class T> struct Out : B {
  T v = 4;
  M w;
  Out();
};
template <class T> Out<T>::Out() {}
template <class... Bs> struct P : Bs... {
  int v = 7;
  P() {}
};
int main() {
  S<int> s;
  S<int> t(0);
  S<int> u('a', 'b');
  O<int>::I i;
  Out<int> o;
  P<B, M> p;
  std::printf("%d %d %d | %d %d %d | %d %d %d | %d %d | %d %d %d | %d %d %d\n", s.v, s.b,
              s.w.m, t.v, t.b, t.w.m, u.v, u.b, u.w.m, i.a, i.c, o.v, o.b, o.w.m, p.v,
              p.b, p.m);
}
