// [over.match.class.deduct]/1.1, [basic.scope.param]: a constructor
// parameter's type, default argument or trailing requires-clause may name
// an earlier parameter (`C(T t, Arr<sizeof(t.g(1))>*)`). In the implicit
// guide it names the guide's parameter, not a member of the deduced
// specialization: for a constructor, a constructor template, a member
// template of a class template specialization and an alias template.
// Reduced from real code.
//@ expect: run
//@ oracle: gcc
//@ stdout: C2 4|C4 2|C5|C5 req|C3 4|I|I req|J|A|Q 2|
#include <cstdio>
template <int N> struct Arr {};
struct S { int g(int) { return 2; } };
struct L { long g(int) { return 3; } };

template <class T> struct C2 {
  C2(T t, Arr<sizeof(t.g(1))>* = nullptr) { std::printf("C2 %d|", int(sizeof(t.g(1)))); }
};
template <class T> struct C4 {
  C4(T t, decltype(t.g(1)) v = 0) { std::printf("C4 %d|", int(v) + t.g(1)); }
};
template <class T> struct C5 {
  C5(T) { std::printf("C5|"); }
  C5(T t, int) requires (sizeof(t) > 0) { std::printf("C5 req|"); }
};
template <class T> struct C3 {
  template <class U> C3(T, U u, Arr<sizeof(u.g(1))>* = nullptr) {
    std::printf("C3 %d|", int(sizeof(u.g(1))));
  }
};
template <class X> struct O {
  template <class T> struct I {
    I(T t, X, Arr<sizeof(t.g(1))>* = nullptr) { std::printf("I|"); }
    I(T t, X, int, decltype(t.g(1)) = 0) requires (sizeof(t) > 0) { std::printf("I req|"); }
  };
  template <class T> struct J {
    template <class U> J(T, U u, Arr<sizeof(u.g(1))>* = nullptr) { std::printf("J|"); }
  };
};
template <class T, class V> struct B {
  B(T t, V, Arr<sizeof(t.g(1))>* = nullptr) { std::printf("A|"); }
};
template <class T> using A = B<T, int>;
template <class T> struct Q {
  template <class... U> Q(T, U... us) requires (sizeof...(us) == 2) {
    std::printf("Q %d|", int(sizeof...(us)));
  }
};

int main() {
  C2 c2(S{});
  static_assert(__is_same(decltype(c2), C2<S>));
  C4 c4(S{});
  static_assert(__is_same(decltype(c4), C4<S>));
  C5 c5(S{});
  static_assert(__is_same(decltype(c5), C5<S>));
  C5 c5r(L{}, 1);
  static_assert(__is_same(decltype(c5r), C5<L>));
  C3 c3(1, S{});
  static_assert(__is_same(decltype(c3), C3<int>));
  O<int>::I i(S{}, 1);
  static_assert(__is_same(decltype(i), O<int>::I<S>));
  O<int>::I ir(L{}, 1, 2);
  static_assert(__is_same(decltype(ir), O<int>::I<L>));
  O<int>::J j(1L, S{});
  static_assert(__is_same(decltype(j), O<int>::J<long>));
  A a(L{}, 1);
  static_assert(__is_same(decltype(a), B<L, int>));
  Q q(1, 'a', 2L);
  static_assert(__is_same(decltype(q), Q<int>));
  std::printf("\n");
}
