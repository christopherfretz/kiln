// [expr.const]/5.1, [implimits]: register locals (spec/oven.md "Register
// locals") change no step count. A selection sort whose locals are kiln
// registers: a swap through std::move and scalars declared without an
// initializer and assigned first. 3145 is the smallest -fconstexpr-steps
// clang 19 and 22 accept (register-locals-over-limit.cpp checks 3144 fails
// on both compilers); only the array is in abstract memory (128 bytes).
//@ args: -fconstexpr-steps=3145
//@ oven: 46 steps<=3145 bytes<=128
//@ oven: 47 steps<=3139 bytes<=128
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
