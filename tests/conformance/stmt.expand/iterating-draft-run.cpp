//@ expect: run
//@ strict-diags
//@ oracle: gcc
//@ requires: clang>=23
//@ stdout: c 1 2 3
//@ stdout: p 4 5 6
//@ stdout: r 30 31 32 / 30
//@ stdout: m 11 12 13
//@ stdout: v 1 2 3 / 1 2 3
//@ stdout: n 11 12 21 22
//@ stdout: b 1 3
//@ stdout: e 0
//@ stdout: t 7 8 9 / 6
//@ stdout: a 5 6
// [stmt.expand]/5.2 (working draft): an iterating expansion statement
// declares `constexpr_opt decltype(auto) range = (init);` and
// `constexpr_opt auto begin = begin-expr;` with automatic storage duration,
// `constexpr` exactly when the for-range-declaration is, and per element
// `constexpr_opt auto iter = begin + decltype(begin - begin){i};`. So a
// non-constexpr for-range-declaration over an lvalue refers to the object
// itself (the body sees and modifies its elements), while N is still a
// constant: the distance from begin-expr to end-expr.
extern "C" int printf(const char *, ...);

template <class T, int N> struct Arr {
  T e[N];
  constexpr T *begin() { return e; }
  constexpr T *end() { return e + N; }
  constexpr const T *begin() const { return e; }
  constexpr const T *end() const { return e + N; }
};

// A class iterator whose difference type is not std::ptrdiff_t.
struct It {
  int v;
  constexpr int operator*() const { return v; }
  constexpr It operator+(int n) const { return {v + n}; }
  constexpr int operator-(It o) const { return v - o.v; }
  constexpr bool operator!=(It o) const { return v != o.v; }
  constexpr It &operator++() { return ++v, *this; } // N's count uses it
};
struct Seq {
  int lo, hi;
  constexpr It begin() const { return {lo}; }
  constexpr It end() const { return {hi}; }
};

constexpr Arr<int, 3> ca{{1, 2, 3}};
Arr<int, 3> make(int b) { return {{b, b + 1, b + 2}}; }

template <class R> void dep(const R &r) {
  printf("t");
  template for (auto x : r) printf(" %d", x);
  int s = 0;
  template for (constexpr auto x : Seq{1, 4}) s += x;
  printf(" / %d\n", s);
}

int main() {
  // A constexpr variable, by a constexpr for-range-declaration.
  printf("c");
  template for (constexpr int x : ca) printf(" %d", x);
  printf("\n");
  // A prvalue whose value is a run-time one: N comes from the type.
  printf("p");
  template for (int x : make(4)) printf(" %d", x);
  printf("\n");
  // A run-time object by reference: the elements are its own.
  Arr<int, 3> r{{30, 31, 32}};
  const int *first = nullptr;
  printf("r");
  template for (const int &x : r) {
    if (!first) first = &x;
    printf(" %d", x);
  }
  printf(" / %d\n", first == &r.e[0] ? 30 : -1);
  // A body that modifies the elements modifies the object.
  template for (int &x : r) x -= 19;
  printf("m %d %d %d\n", r.e[0], r.e[1], r.e[2]);
  // By value: the copy is the loop variable's, the object is unchanged.
  Arr<int, 3> v{{1, 2, 3}};
  printf("v");
  template for (int x : v) {
    x *= 10;
    (void)x;
  }
  template for (auto x : v) printf(" %d", x);
  printf(" /");
  template for (auto x : v) printf(" %d", x);
  printf("\n");
  // Nested expansions over constant ranges.
  printf("n");
  template for (constexpr auto o : Seq{1, 3}) {
    template for (constexpr auto i : Seq{1, 3}) printf(" %d", o * 10 + i);
  }
  printf("\n");
  // continue ends the element, break leaves the statement.
  printf("b");
  template for (constexpr auto x : Seq{1, 6}) {
    if (x == 2) continue;
    if (x == 4) break;
    printf(" %d", x);
  }
  printf("\n");
  // An empty range: no element is instantiated.
  int count = 0;
  template for (constexpr auto x : Seq{3, 3}) count += x;
  printf("e %d\n", count);
  // A dependent range in a template.
  dep(make(7));
  // An array is destructured ([stmt.expand]/4.3), a run-time one too.
  int a[2] = {5, 6};
  printf("a");
  template for (auto x : a) printf(" %d", x);
  printf("\n");
}
