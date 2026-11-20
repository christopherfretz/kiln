// [over.match.oper], [temp.res]: every overloaded-operator form resolved
// in a template definition (a non-dependent operand) is formed again
// when the template is instantiated: the unary operators, postfix
// `++`/`--`, every binary and compound-assignment operator, `&&`, `||`,
// `,`, `->*`, `()`, `[]` with zero, one and two operands, `->`, and the
// comparisons rewritten to `==` or `<=>` ([over.match.oper]/3.4, /8),
// with P2468's rule that an `operator==` with a matching `operator!=`
// is not rewritten. The operands that are dependent take the same path.
// WP-1738 (GAP-054, GAP-058).
//@ expect: run
//@ stdout: 7406 1258|218 37|
#include <compare>
#include <cstdio>
struct V {
  int v;
  V operator+() const { return {v + 100}; }
  V operator-() const { return {-v}; }
  int operator*() const { return v * 2; }
  V *operator&() { return this; }
  int operator~() const { return ~v; }
  bool operator!() const { return v == 0; }
  V &operator++() { ++v; return *this; }
  V operator++(int) { V o = *this; ++v; return o; }
  V &operator--() { --v; return *this; }
  V operator--(int) { V o = *this; --v; return o; }
  V operator+(V o) const { return {v + o.v}; }
  V operator-(V o) const { return {v - o.v}; }
  V operator*(V o) const { return {v * o.v}; }
  V operator/(V o) const { return {v / o.v}; }
  V operator%(V o) const { return {v % o.v}; }
  V operator^(V o) const { return {v ^ o.v}; }
  V operator&(V o) const { return {v & o.v}; }
  V operator|(V o) const { return {v | o.v}; }
  V operator<<(int s) const { return {v << s}; }
  V operator>>(int s) const { return {v >> s}; }
  V &operator=(int x) { v = x; return *this; }
  V &operator+=(V o) { v += o.v; return *this; }
  V &operator-=(V o) { v -= o.v; return *this; }
  V &operator*=(V o) { v *= o.v; return *this; }
  V &operator/=(V o) { v /= o.v; return *this; }
  V &operator%=(V o) { v %= o.v; return *this; }
  V &operator^=(V o) { v ^= o.v; return *this; }
  V &operator&=(V o) { v &= o.v; return *this; }
  V &operator|=(V o) { v |= o.v; return *this; }
  V &operator<<=(int s) { v <<= s; return *this; }
  V &operator>>=(int s) { v >>= s; return *this; }
  bool operator&&(V o) const { return v && o.v; }
  bool operator||(V o) const { return v || o.v; }
  int operator,(V o) const { return v * 1000 + o.v; }
  int operator->*(int k) const { return v + k; }
  int operator()(int a, int b) const { return v + a + b; }
  int operator()() const { return v; }
  int operator[](int i) const { return v * i; }
  int operator[](int i, int j) const { return v + i * j; }
  int operator[]() const { return -1; }
  V *operator->() { return this; }
  std::strong_ordering operator<=>(const V &) const = default;
  bool operator==(const V &) const = default;
};
struct E {  // operator== only: != and reversed == are rewritten
  int v;
  bool operator==(int x) const { return v == x; }
};
struct N {  // P2468: == with a matching != is not rewritten
  int v;
  bool operator==(const N &o) const { return v == o.v; }
  bool operator!=(const N &o) const { return v != o.v + 1; }
};
struct C {  // <=> only, against int: <, >, <=, >= and reversed forms rewritten
  int v;
  std::strong_ordering operator<=>(int x) const { return v <=> x; }
  bool operator==(int x) const { return v == x; }
};
template <class T> int all(T seed) {
  V a{seed}, b{3};
  int r = 0;
  r += (+a).v + (-a).v + *a + (&a)->v + ~a + !a;
  r += (++a).v; r += (a++).v; r += (--a).v; r += (a--).v;
  r += (a + b).v + (a - b).v + (a * b).v + (a / b).v + (a % b).v;
  r += (a ^ b).v + (a & b).v + (a | b).v + (a << 2).v + (a >> 1).v;
  V c{0}; c = 9; r += c.v;
  c += b; c -= a; c *= b; c /= b; c %= b; c ^= a; c &= b; c |= a; c <<= 1; c >>= 1; r += c.v;
  r += (a && b) + (a || V{0});
  r += (a, b);
  r += a->*5;
  r += a(1, 2) + a() + a[4] + a[2, 3] + a[];
  r += a->v;
  r += (a < b) + (a > b) + (a <= b) + (a >= b) + (a == b) + (a != b);
  E e{seed};
  r += (e != 5) * 10 + (5 == e) * 20 + (5 != e) * 40;
  N n{seed}, m{seed - 1};
  r += (n != m) * 80 + (n == m) * 160;
  C k{seed};
  r += (k < 7) + (7 < k) * 2 + (k >= 7) * 4 + (7 >= k) * 8 + (7 == k) * 16 + (7 != k) * 32;
  return r;
}
template <class T, class U> int dep(T e, U k) {
  int r = (e != 5) + (5 == e) * 2 + (5 != e) * 4 + (e == 5) * 8;
  r += (k < 7) * 16 + (7 < k) * 32 + (k <= 7) * 64 + (7 >= k) * 128;
  return r;
}
int main() {
  std::printf("%d %d|", all(7), all(1));
  std::printf("%d %d|\n", dep(E{5}, C{6}), dep(E{4}, C{9}));
}
