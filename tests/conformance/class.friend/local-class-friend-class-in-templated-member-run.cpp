// [class.friend]/11, [temp.friend]: a friend class of a local class in a
// member function of a class template, or in a generic lambda in a
// function template, belongs to that block scope; each instantiation
// declares it again in the instantiated block, where the class defined
// later in the block is the friend. The -denied sibling is the class of a
// nested block, which is not.
//@ skip(clang): clang searches past the block scope and does not befriend a class defined later in it
//@ oracle: gcc
//@ expect: run
//@ std: c++20
//@ stdout: 4 2 3 1

#include <cstdio>

template <class T> struct S {
  int m() {
    class L { friend class F; int i = sizeof(T); };
    class F { public: int g(L &l) { return l.i; } };
    L l;
    return F().g(l);
  }
};

template <class T> int f() {
  auto g = [](auto x) {
    struct L { friend struct Z; int i = 2 * sizeof(x); };
    struct Z { int g(L &l) { return l.i; } };
    L l;
    return Z().g(l);
  };
  return g(T());
}

template <class T> struct O {
  struct M {
    int m() {
      struct L { friend struct Z; int i = 3; };
      struct Z { int g(L &l) { return l.i; } };
      L l;
      return Z().g(l);
    }
  };
};

int main() {
  std::printf("%d %d %d %d\n", S<int>().m(), f<char>(), O<int>::M().m(),
              S<char>().m());
}
