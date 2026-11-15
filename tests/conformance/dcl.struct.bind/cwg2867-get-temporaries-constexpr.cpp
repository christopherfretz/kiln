// The constant-evaluation twin of cwg2867-get-temporaries-run.cpp: the
// temporaries of a tuple-like structured binding's initializer persist
// until the last `get` call is done ([dcl.struct.bind]/4, CWG2867), each
// `get` call's own temporaries die after it, and one a holding variable
// extends lives as long as the binding, so reading it afterwards is not a
// constant expression.
//@ expect: compile-fail
//@ strict-diags
//@ xfail(clang): neither clang 19 nor clang 22 implements CWG2867
//@ diag: 69:15 error "constexpr variable 'z' must be initialized by a constant expression"
//@ diag: 67:10 note "read of object outside its lifetime is not allowed in a constant expression"
//@ diag: 69:19 note "in call to 'dangling()'"
namespace std {
template <class T> struct tuple_size;
template <unsigned long I, class T> struct tuple_element;
}
struct Log { int v[32] = {}; int n = 0; constexpr void add(int x) { v[n++] = x; } };
struct T {
  Log* l; int id;
  constexpr T(Log* l, int id) : l(l), id(id) { l->add(id); }
  constexpr T(const T& o) : l(o.l), id(o.id + 100) { l->add(id); }
  constexpr ~T() { l->add(-id); }
};
struct P {
  Log* l; int a, b;
  template <int I> constexpr int get() const { T g(l, 50 + I); return I ? b : a; }
};
template <> struct std::tuple_size<P> { static constexpr int value = 2; };
template <unsigned long I> struct std::tuple_element<I, P> { using type = int; };
constexpr P make(Log* l, const T&) { return P{l, 7, 8}; }
constexpr bool run() {
  Log l;
  {
    auto [x, y] = make(&l, T(&l, 1));
    l.add(x + y);
  }
  // T1, then each get call's own T, then ~T1, then the body.
  int e[] = {1, 50, -50, 51, -51, -1, 15};
  for (int k = 0; k < 7; ++k) if (l.v[k] != e[k]) return false;
  return l.n == 7;
}
static_assert(run());
struct Q {
  Log* l;
  template <int I> constexpr T get() const { return T(l, 60 + I); }
};
template <> struct std::tuple_size<Q> { static constexpr int value = 2; };
template <unsigned long I> struct std::tuple_element<I, Q> { using type = T; };
constexpr bool run2() {
  Log l;
  {
    auto&& [x, y] = Q{&l};
    l.add(x.id + y.id);
  }
  int e[] = {60, 61, 121, -61, -60};
  for (int k = 0; k < 5; ++k) if (l.v[k] != e[k]) return false;
  return l.n == 5;
}
static_assert(run2());
constexpr int dangling() {
  Log l;
  const int* p;
  {
    auto&& [x, y] = Q{&l};
    p = &x.id;
  }
  return *p;
}
constexpr int z = dangling();
