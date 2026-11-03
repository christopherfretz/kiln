// [stmt.expand], [temp.inst]: an expansion statement in a template is
// expanded when the enclosing template is instantiated: over a dependent
// type (whose form is only known then), a pack, in member functions of
// class templates (in-class and out-of-class, and an explicit
// specialization), a member template, a generic lambda, and a local class.
//@ expect: compile-ok
//@ strict-diags
//@ requires: clang>=23
#include <array>
#include <tuple>

template <class T> constexpr int sum(const T& t) {
  int s = 0;
  template for (auto x : t) { s += x; }
  return s;
}
static_assert(sum(std::tuple<int, int>{3, 4}) == 7);
int arr[2] = {1, 2};
int use_arr() { return sum(arr); }

template <class T> constexpr unsigned long sizes() {
  unsigned long n = 0;
  template for (constexpr auto x : T{}) { n += sizeof(x); }
  return n;
}
struct P { int a; char b; };
static_assert(sizes<std::tuple<int, char, long>>() == sizeof(int) + 1 + sizeof(long));
static_assert(sizes<P>() == sizeof(int) + 1);
static_assert(sizes<std::array<int, 3>>() == 3 * sizeof(int));

template <int... Is> constexpr int pack() {
  int s = 0;
  template for (constexpr int i : {Is...}) {
    static_assert(i > 0);
    s += i;
  }
  return s;
}
static_assert(pack<1, 2, 3>() == 6);

template <class T> struct A {
  template <class U> static constexpr int f(U u) {
    int s = 0;
    template for (auto x : u) {
      template for (auto y : {T(1), T(2)}) { s += x * y; }
    }
    return s;
  }
  static constexpr int g() {
    int s = 0;
    template for (constexpr T x : {T(1), T(2)}) {
      auto l = [&](auto z) { template for (auto w : {z, z}) { s += w * x; } };
      l(x);
    }
    return s;
  }
  int h(T);
};
static_assert(A<int>::f(std::tuple<int, int>{1, 2}) == 9);
static_assert(A<int>::g() == 1 + 1 + 4 + 4);

template <class T> int A<T>::h(T t) {
  int s = 0;
  template for (auto x : t) { s += x; }
  return s;
}
template <> int A<int>::h(int t) {
  int s = 0;
  template for (auto x : {t, t}) { s += x; }
  return s;
}

template <class T> int local(T t) {
  struct L {
    static int g(T t) {
      int s = 0;
      template for (auto x : t) { s += x; }
      return s;
    }
  };
  return L::g(t);
}

int use() {
  return A<std::tuple<int, int>>{}.h({1, 2}) + A<int>{}.h(5) +
         local(std::tuple<int, int, int>{1, 1, 1});
}
