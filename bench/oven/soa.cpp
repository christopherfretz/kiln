//@ args: -fconstexpr-steps=100000000
// Oven struct-of-arrays (spec/oven.md "Class benchmarks"): the shape of
// mirrorbind's compile-time binding tables. A table of 4096 member records
// is kept as parallel column objects (ids, offsets, sizes, flags) inside
// one class; it is filled through member functions, sorted by key with an
// index permutation, queried by binary search, and folded into a checksum.
// Exercises class objects with array members, member calls through `this`
// and references, and nested aggregates (WP-076). The gated copy is
// tests/conformance/class.mem/soa-table.cpp.

constexpr int N = 4096;

struct Column {
  unsigned v[N] = {};
  constexpr unsigned &ref(int i) { return v[i]; }
  constexpr unsigned at(int i) const { return v[i]; }
};

struct Flags {
  bool pub[N] = {};
  bool stat[N] = {};
};

struct Table {
  Column id, offset, size;
  Flags flags;
  int perm[N] = {};
  int n = 0;

  constexpr void push(unsigned key, unsigned off, unsigned sz, bool pub, bool st) {
    id.ref(n) = key;
    offset.ref(n) = off;
    size.ref(n) = sz;
    flags.pub[n] = pub;
    flags.stat[n] = st;
    perm[n] = n;
    ++n;
  }

  constexpr unsigned key(int i) const { return id.at(perm[i]); }

  // Shell sort of the permutation by id.
  constexpr void sort() {
    for (int gap = n / 2; gap > 0; gap /= 2)
      for (int i = gap; i < n; ++i) {
        int t = perm[i];
        int j = i;
        for (; j >= gap && id.at(perm[j - gap]) > id.at(t); j -= gap) perm[j] = perm[j - gap];
        perm[j] = t;
      }
  }

  constexpr int find(unsigned k) const {
    int lo = 0, hi = n;
    while (lo < hi) {
      int mid = (lo + hi) / 2;
      if (key(mid) < k) lo = mid + 1;
      else hi = mid;
    }
    return lo < n && key(lo) == k ? perm[lo] : -1;
  }
};

constexpr void fill(Table &t) {
  unsigned seed = 12345u, off = 0;
  for (int i = 0; i < N; ++i) {
    seed = seed * 1664525u + 1013904223u;
    unsigned sz = 1u + (seed >> 28);
    t.push(seed >> 4, off, sz, (seed & 1) != 0, (seed & 2) != 0);
    off += sz;
  }
}

constexpr unsigned checksum() {
  Table t;
  fill(t);
  t.sort();
  unsigned sum = 0;
  for (int i = 0; i < t.n; ++i) {
    int r = t.find(t.id.at(i));
    if (r < 0) return 0;
    unsigned w = t.offset.at(r) * 31u + t.size.at(r);
    if (t.flags.pub[r]) w ^= 0x9e3779b9u;
    if (t.flags.stat[r]) w += 7u;
    sum = sum * 33u + w;
  }
  return sum;
}

static_assert(checksum() == 3160698304u);
