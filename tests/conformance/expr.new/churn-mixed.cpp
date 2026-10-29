// [expr.new], [expr.delete], [expr.const]/5.20 + kiln Oven budgets
// (spec/oven.md "Churn benchmarks"): 10 000 operations interleaving
// 8/24/40/200-byte allocations that are freed in a different order than
// allocated; the size classes keep the address space within twice the live
// bytes. The same program as bench/oven/churn-mixed.cpp; the gates are the
// spec's design targets, set from the live-object count.
//@ args: -fconstexpr-steps=100000000
//@ oven: 38 peak_bytes<=32768 peak_arena<=65536
// 10 000 operations on 64 slots chosen by an LCG: an empty slot gets an
// 8, 24, 40 or 200-byte allocation, a full one is freed, so blocks are
// freed in a different order than they were allocated.
constexpr int churn(int ops) {
  char *slot[64] = {};
  int size[64] = {};
  unsigned x = 12345;
  int live = 0, made = 0;
  for (int k = 0; k < ops; ++k) {
    x = x * 1664525u + 1013904223u;
    int i = (x >> 10) % 64;
    if (slot[i]) {
      if (slot[i][size[i] - 1] != static_cast<char>(i)) return -1;
      delete[] slot[i];
      slot[i] = nullptr;
      --live;
    } else {
      const int sizes[4] = {8, 24, 40, 200};
      size[i] = sizes[(x >> 20) % 4];
      slot[i] = new char[size[i]];
      slot[i][size[i] - 1] = static_cast<char>(i);
      ++live;
      ++made;
    }
  }
  for (int i = 0; i < 64; ++i) delete[] slot[i];
  return made;
}

static_assert(churn(10000) > 0);
