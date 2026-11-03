// [stmt.expand]/5.1: an expansion-init-list `{ e0, ..., eN-1 }` expands the
// body once per expression, the for-range-declaration of the i-th copy
// initialized from e_i. Each element may have its own type; `constexpr`
// makes each element variable constexpr ([stmt.expand]/4), usable in a
// static_assert and as a template argument inside the body.
//@ expect: run
//@ strict-diags
//@ requires: clang>=23
//@ stdout: 1 2.5 c
//@ stdout: 6
extern "C" int printf(const char*, ...);

template <int N> struct Tag { static constexpr int value = N; };

constexpr int sum() {
  int s = 0;
  template for (constexpr int i : {1, 2, 3}) {
    static_assert(i >= 1 && i <= 3);
    s += Tag<i>::value;
  }
  return s;
}
static_assert(sum() == 6);

constexpr unsigned long sizes() {
  unsigned long n = 0;
  template for (auto x : {'a', short(1), 2, 3L}) { n += sizeof(x); }
  return n;
}
static_assert(sizes() == sizeof(char) + sizeof(short) + sizeof(int) + sizeof(long));

void print(int a, double b, char c) {
  int i = 0;
  template for (auto x : {a, b, c}) {
    if constexpr (__is_same(decltype(x), int)) printf("%d", x);
    else if constexpr (__is_same(decltype(x), double)) printf(" %.1f", x);
    else printf(" %c\n", x);
    ++i;
  }
}

int main() {
  print(1, 2.5, 'c');
  printf("%d\n", sum());
  return 0;
}
