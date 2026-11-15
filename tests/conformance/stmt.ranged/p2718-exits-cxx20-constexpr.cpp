// The C++20 twin of p2718-exits-constexpr.cpp: before P2718R0 only the
// temporary bound to `__range` is extended, so `items()`'s object is
// destroyed at the end of the for-range-initializer and the loop's
// `begin()` call is not a constant expression ([class.temporary]/7).
//@ expect: compile-fail
//@ strict-diags
//@ std: c++20
//@ diag: 55:16 error "constexpr variable 'b' must be initialized by a constant expression"
//@ diag: 40:14 note "member call on object outside its lifetime is not allowed in a constant expression"
//@ diag: 49:3 note "in call to 'loop(l, 0)'"
//@ diag: 55:20 note "in call to 'exits()'"
struct Log {
  int v[40] = {};
  int n = 0;
  constexpr void add(int x) { v[n++] = x; }
  constexpr bool is(const int* e, int k) const {
    for (int i = 0; i < k; ++i)
      if (v[i] != e[i]) return false;
    return n == k;
  }
};
struct T {
  Log* l; int id;
  constexpr T(Log* l, int id) : l(l), id(id) { l->add(id); }
  constexpr ~T() { l->add(-id); }
};
struct L {
  Log* l; int a[2] = {1, 2};
  constexpr ~L() { l->add(-9); }
  constexpr const int* begin() const { return a; }
  constexpr const int* end() const { return a + 2; }
};
struct W {
  L x;
  constexpr const L& items() const& { return x; }
};
constexpr W wrap(const T& t) { return W{{t.l}}; }

constexpr int loop(Log& l, int stop) {
  for (int x : wrap(T(&l, 7)).items()) {
    l.add(x);
    if (x == stop) return x;
    if (x == -stop) break;
  }
  return 0;
}
constexpr bool exits() {
  Log l;
  loop(l, 0);
  loop(l, 1);
  loop(l, -2);
  const int e[] = {7, 1, 2, -9, -7, 7, 1, -9, -7, 7, 1, 2, -9, -7};
  return l.is(e, 14);
}
constexpr bool b = exits();
