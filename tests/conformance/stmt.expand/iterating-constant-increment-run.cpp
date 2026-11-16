//@ expect: run
//@ strict-diags
//@ requires: clang>=23
//@ oracle: gcc
//@ stdout: 0 0 12 12
// [stmt.expand]/5.2: the count's `++b` runs once per element, so an empty
// range never evaluates it and a non-constexpr `operator++` is then
// accepted, with or without `constexpr`; a constexpr one is evaluated
// (the rejected side: `iterating-constant-increment-neg.cpp`). GCC 16.1
// agrees.
#include <cstdio>

constexpr int a[] = {3, 4, 5};
struct It {
  const int *p;
  constexpr int operator*() const { return *p; }
  constexpr It operator+(long n) const { return {p + n}; }
  constexpr long operator-(It o) const { return p - o.p; }
  constexpr bool operator!=(It o) const { return p != o.p; }
  It &operator++() { ++p; return *this; }
};
struct Kt {
  const int *p;
  constexpr int operator*() const { return *p; }
  constexpr Kt operator+(long n) const { return {p + n}; }
  constexpr long operator-(Kt o) const { return p - o.p; }
  constexpr bool operator!=(Kt o) const { return p != o.p; }
  constexpr Kt &operator++() { ++p; return *this; }
};
struct Empty { constexpr It begin() const { return {a}; } constexpr It end() const { return {a}; } };
struct Full { constexpr Kt begin() const { return {a}; } constexpr Kt end() const { return {a + 3}; } };

int main() {
  static constexpr Empty e;
  static constexpr Full g;
  int s0 = 0, s1 = 0, s2 = 0, s3 = 0;
  template for (constexpr int x : e) s0 += x;
  template for (int x : e) s1 += x;
  template for (constexpr int x : g) s2 += x;
  template for (int x : g) s3 += x;
  std::printf("%d %d %d %d\n", s0, s1, s2, s3);
}
