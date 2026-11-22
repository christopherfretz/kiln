// [expr.const]/5.1, [implimits]: one step below the minimum
// register-locals.cpp needs: the evaluation stops at the step limit on both
// kiln and clang (the note shows the parameter's current value, as clang's).
//@ args: -fconstexpr-steps=3144
//@ expect: compile-fail
//@ strict-diags
//@ diag: 46:15 error "static assertion expression is not an integral constant expression"
//@ diag: 44:3 note "constexpr evaluation hit maximum step limit; possible infinite loop?"
//@ diag: 46:15 note "in call to 'selection_sort_checksum(761386273)'"
#include <utility>
template <class T> constexpr void exchange_values(T &a, T &b) {
  T t = std::move(a);
  a = std::move(b);
  b = std::move(t);
}
constexpr unsigned selection_sort_checksum(unsigned seed) {
  unsigned a[32] = {};
  for (int i = 0; i < 32; ++i) {
    seed = seed * 1664525u + 1013904223u;
    a[i] = seed >> 8;
  }
  for (int i = 0; i < 32; ++i) {
    int best;
    best = i;
    for (int j = i + 1; j < 32; ++j) {
      unsigned v;
      if (a[j] < a[best])
        v = 1;
      else
        v = 0;
      if (v)
        best = j;
    }
    exchange_values(a[i], a[best]);
  }
  unsigned h = 0;
  for (int i = 0; i < 32; ++i) {
    unsigned prev;
    prev = i ? a[i - 1] : 0;
    if (prev > a[i])
      return 0;
    h = h * 31u + a[i];
  }
  return h;
}
static_assert(selection_sort_checksum(1) != 0);
static_assert(selection_sort_checksum(7) != 0);
