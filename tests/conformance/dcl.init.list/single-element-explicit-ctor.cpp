// [over.match.list], CWG2311 as clang implements it: b({A()}) picks B(A)
// over B(std::vector<A>). From clang's SemaCXX/single-element-init-list.cpp.
// Found by the clang negative-test run (WP-912).
//@ std: c++17
//@ expect: compile-ok
namespace std {
typedef decltype(sizeof(int)) size_t;
template <class E> class initializer_list {
  const E *b;
  size_t n;
  constexpr initializer_list(const E *b, size_t n) : b(b), n(n) {}

public:
  constexpr initializer_list() : b(nullptr), n(0) {}
  constexpr size_t size() const { return n; }
};
template <typename T> struct vector {
  size_t sz;
  constexpr vector(initializer_list<T> ilist) : sz(ilist.size()) {}
  constexpr vector(const vector &other) : sz(other.sz) {}
};
} // namespace std
struct A {
  constexpr A() {}
};
struct B {
  int called_ctor;
  constexpr explicit B(A) : called_ctor(0) {}
  constexpr explicit B(std::vector<A>) : called_ctor(1) {}
};
struct C {
  B b;
  constexpr C() : b({A()}) {}
};
static_assert(C().b.called_ctor == 0, "");
