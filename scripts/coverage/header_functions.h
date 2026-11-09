// Header of header_functions.cpp (scripts/coverage-compare.py).
#pragma once
inline int clamp_add(int a, int b, int hi) {
  int s = a + b;
  if (s > hi)
    return hi;
  return s;
}
template <class T> T twice(T v) { return v + v; }
inline int never_called(int x) { return x ? 1 : 2; }
struct Acc {
  int total = 0;
  void add(int v) {
    if (v > 0)
      total += v;
  }
  int unused_member() const { return total * 2; }
};
