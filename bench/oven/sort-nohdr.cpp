//@ args: -fconstexpr-steps=200000000
// sort.cpp without libc++: insertion sort (element swaps through
// references) and a hand-written heap sort in place of std::sort, over
// plain arrays passed by pointer, checked against each other and a
// checksum; then a 4000-element heap sort.

constexpr void lcg_fill(unsigned *a, int n, unsigned seed) {
  for (int i = 0; i < n; ++i) {
    seed = seed * 1664525u + 1013904223u;
    a[i] = seed >> 8;
  }
}

constexpr void swap(unsigned &x, unsigned &y) {
  unsigned t = x;
  x = y;
  y = t;
}

constexpr void insertion_sort(unsigned *a, int n) {
  for (int i = 1; i < n; ++i)
    for (int j = i; j > 0 && a[j - 1] > a[j]; --j) swap(a[j - 1], a[j]);
}

constexpr void sift_down(unsigned *a, int root, int end) {
  while (2 * root + 1 < end) {
    int child = 2 * root + 1;
    if (child + 1 < end && a[child] < a[child + 1]) ++child;
    if (a[root] >= a[child]) return;
    swap(a[root], a[child]);
    root = child;
  }
}

constexpr void heap_sort(unsigned *a, int n) {
  for (int i = n / 2 - 1; i >= 0; --i) sift_down(a, i, n);
  for (int end = n - 1; end > 0; --end) {
    swap(a[0], a[end]);
    sift_down(a, 0, end);
  }
}

constexpr bool is_sorted(const unsigned *a, int n) {
  for (int i = 1; i < n; ++i)
    if (a[i - 1] > a[i]) return false;
  return true;
}

constexpr bool check(unsigned seed, int n) {
  unsigned in[400] = {};
  unsigned a[400] = {};
  unsigned b[400] = {};
  lcg_fill(in, n, seed);
  lcg_fill(a, n, seed);
  lcg_fill(b, n, seed);
  insertion_sort(a, n);
  heap_sort(b, n);
  unsigned long long sum_in = 0, sum_out = 0;
  for (int i = 0; i < n; ++i) {
    if (a[i] != b[i]) return false;
    sum_in += in[i];
    sum_out += a[i];
  }
  return is_sorted(a, n) && sum_in == sum_out;
}

static_assert(check(1, 16));
static_assert(check(42, 400));

constexpr bool big(unsigned seed) {
  unsigned a[4000] = {};
  lcg_fill(a, 4000, seed);
  heap_sort(a, 4000);
  return is_sorted(a, 4000);
}
static_assert(big(7));
