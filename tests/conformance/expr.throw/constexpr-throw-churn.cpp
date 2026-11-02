// [expr.const]/10 (C++26, P3068R6), [except.throw]/4 + kiln Oven budgets
// (spec/oven.md "Exceptions", the WP-060 reclaim rule): 100000 throws in a
// loop, each a 64-byte exception object whose constructor allocates and
// whose destructor frees a heap buffer, thrown through two frames with an
// automatic object to destroy on the way. Every exception object, buffer
// and frame is reclaimed as the evaluation goes: the heap and arena peak at
// one round, not 100000.
//@ skip(clang): neither clang 19 nor clang 22 implements P3068
//@ args: -fconstexpr-steps=100000000
//@ oven: 49 allocs<=133332 peak_bytes<=256 peak_arena<=1024
//@ mem<=512
struct Err {
  int code;
  int *buf;
  long long pad[6];
  constexpr explicit Err(int c) : code(c), buf(new int[4]{c, c, c, c}), pad{} {}
  constexpr Err(const Err &o) : code(o.code), buf(new int[4]{o.code, 0, 0, 0}), pad{} {}
  constexpr ~Err() { delete[] buf; }
};

struct Guard {
  int *count;
  constexpr ~Guard() { ++*count; }
};

constexpr void deep(int i, int *count) {
  Guard g{count};
  if (i % 3 != 0) throw Err(i);
}

constexpr void mid(int i, int *count) {
  Guard g{count};
  deep(i, count);
}

constexpr long long churn(int rounds) {
  long long sum = 0;
  int guards = 0;
  for (int i = 0; i < rounds; ++i) {
    try {
      mid(i, &guards);
    } catch (const Err &e) {
      sum += e.buf[0];
    }
  }
  return sum + guards;
}

static_assert(churn(100000) == 3333266667LL + 200000);
