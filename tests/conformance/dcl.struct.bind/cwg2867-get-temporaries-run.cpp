// CWG2867, [dcl.struct.bind]/4, [intro.execution]/5: the initialization
// of the variables a tuple-like structured binding declaration introduces
// (the `get` calls) is part of the declaration's full-expression, so the
// temporaries of the initializer (here a temporary argument `get` reads
// through `e`) persist until every `get` has returned; each `get` call's
// own temporaries (its default argument) are destroyed after it, the
// temporaries `get` results are bound to persist with the binding, and
// they are destroyed before `e`. For `auto&& [..]` the extended object is
// the hidden `e`; for `auto [..]` it is a variable destroyed at the end
// of its scope. Block-scope automatic, static and thread-local
// declarations and a namespace-scope one; array and aggregate structured
// bindings only extend `e`. Found running GCC's C++ testsuite.
//@ expect: run
//@ std: c++26
//@ strict-diags
//@ oracle: gcc
//@ xfail(clang): neither clang 19 nor clang 22 implements CWG2867 (the initializer's temporaries are destroyed before the get calls)
//@ stdout: main B9 g0(9) ~C g1(9) ~C ~B9 B1 g0(1) ~C g1(1) ~C ~B1 u3 ~V ~V ~E1 | B2 g0(2) ~C g1(2) ~C ~B2 u5 ~V ~V ~E2 | B3 g0(3) ~C g1(3) ~C ~B3 s7 | B4 g0(4) ~C g1(4) ~C ~B4 t9 | B5 a11 ~A5 ~B5 | B6 B7 a13 ~B7 ~B6 | n19 | end ~V ~V ~E4 ~V ~V ~E3 ~V ~V ~E9 |
extern "C" int printf(const char*, ...);
struct Last {
  ~Last() { printf("|\n"); }
} last;
namespace std {
template <class T> struct tuple_size;
template <unsigned long I, class T> struct tuple_element;
}
struct B {
  int v;
  B(int v) : v(v) { printf("B%d ", v); }
  ~B() { printf("~B%d ", v); }
};
struct C {
  ~C() { printf("~C "); }
};
struct V {
  int v;
  ~V() { printf("~V "); }
};
struct E {
  const B* b;
  int id;
  ~E() { printf("~E%d ", id); }
  template <unsigned long I> V get(const C& = C{}) const {
    printf("g%lu(%d) ", I, b->v);
    return V{b->v + int(I)};
  }
};
template <> struct std::tuple_size<E> { static constexpr unsigned long value = 2; };
template <unsigned long I> struct std::tuple_element<I, E> { using type = V; };
template <> struct std::tuple_size<const E> { static constexpr unsigned long value = 2; };
template <unsigned long I> struct std::tuple_element<I, const E> { using type = V; };

E make(const B& b) { return E{&b, b.v}; }

void automatic() {
  auto&& [x, y] = make(B(1));
  printf("u%d ", x.v + y.v);
}
void by_value() {
  auto [x, y] = make(B(2));
  printf("u%d ", x.v + y.v);
}
void statics() {
  static auto [x, y] = make(B(3));
  printf("s%d ", x.v + y.v);
}
void threads() {
  thread_local auto [x, y] = make(B(4));
  printf("t%d ", x.v + y.v);
}
struct A {
  int a, b;
  const B& r;
  ~A() { printf("~A%d ", r.v); }
};
void aggregate() {
  auto&& [p, q, r] = A{5, 6, B(5)};
  printf("a%d ", p + q);
}
void array() {
  using Arr = B[2];
  auto&& [m, n] = Arr{B(6), B(7)};
  printf("a%d ", m.v + n.v);
}
auto [nx, ny] = (printf("main "), make(B(9)));
int main() {
  automatic();
  printf("| ");
  by_value();
  printf("| ");
  statics();
  printf("| ");
  threads();
  printf("| ");
  aggregate();
  printf("| ");
  array();
  printf("| ");
  printf("n%d ", nx.v + ny.v);
  printf("| end ");
}
