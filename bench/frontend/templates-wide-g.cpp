//@ mode: emit-llvm
//@ args: -g
// templates_wide.cpp at `-g` (WP-817): the same 10,000 `Leaf`, 10,000
// `Pair` and 100 `Row` specializations, emitted as IR with debug
// information. One global of the outermost type makes codegen describe
// every specialization (a DICompositeType with its members, a static
// member and template parameters each), so the metadata is about 13x the
// code: 1.1 MB of IR without `-g`, 14 MB with it. Tracks the front-end
// cost of building, uniquing and printing debug metadata
// (docs/status/debuginfo.md "Cost of `-g`").

template <int N> struct Leaf {
  static constexpr int value = N;
  int data[N % 7 + 1];
  char tag;
};

template <int A, int B> struct Pair {
  Leaf<A * 100 + B> leaf;
  static_assert(sizeof(Leaf<A * 100 + B>) > 0);
};

#define KILN_X10(m, a, d) m(a, d##0) m(a, d##1) m(a, d##2) m(a, d##3) m(a, d##4) \
  m(a, d##5) m(a, d##6) m(a, d##7) m(a, d##8) m(a, d##9)
#define KILN_X100(m, a) KILN_X10(m, a, ) KILN_X10(m, a, 1) KILN_X10(m, a, 2) \
  KILN_X10(m, a, 3) KILN_X10(m, a, 4) KILN_X10(m, a, 5) KILN_X10(m, a, 6)       \
  KILN_X10(m, a, 7) KILN_X10(m, a, 8) KILN_X10(m, a, 9)
#define KILN_PAIR(a, b) Pair<a, b> p##b;

template <int A> struct Row {
  KILN_X100(KILN_PAIR, A)
};

#define KILN_ROW(a, b) Row<b> r##b;
struct All {
  KILN_X100(KILN_ROW, 0)
};

All all;
static_assert(sizeof(All) > 10000);
