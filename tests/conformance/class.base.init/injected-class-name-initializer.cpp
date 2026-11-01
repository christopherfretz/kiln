// [class.base.init]/2, [temp.local]/1, /3: in a class template, a
// mem-initializer-id naming the class itself (its injected-class-name)
// delegates ([class.base.init]/6), also after instantiation; a base may be
// named by an injected-class-name inherited from it or by a member
// typedef (libc++ `pair(...) : pair(...)` and `__base(...)`). Matches clang 19.
//@ expect: run
template <class T> struct Pair {
  T a, b;
  constexpr Pair(T x, T y) : a(x), b(y) {}
  constexpr Pair(T x) : Pair(x, x + 1) {}
  template <class U> constexpr Pair(U* p) : Pair(*p) {}
};
static_assert(Pair<int>(3).b == 4);
constexpr int seven = 7;
static_assert(Pair<int>(&seven).a == 7);

template <class T> struct B {
  T v;
  constexpr B(T x) : v(x) {}
};
struct D : B<int> {
  constexpr D() : B(1) {}
};
static_assert(D().v == 1);

template <class T> struct E : B<T> {
  using base = B<T>;
  constexpr E() : base(2) {}
};
static_assert(E<int>().v == 2);

struct F : B<long> {
  typedef B<long> base_type;
  constexpr F() : base_type(5) {}
};
static_assert(F().v == 5);

int main() { return Pair<int>(3).b + D().v + E<int>().v == 7 ? 0 : 1; }
