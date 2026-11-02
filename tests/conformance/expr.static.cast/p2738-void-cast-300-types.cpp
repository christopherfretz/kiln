// [expr.static.cast]/14, [expr.const]/5.14 (C++26, P2738R1): the
// designator of a `void *` stored to memory is kept however many distinct
// types are stored. 300 pointers to 300 distinct types go through a
// `void *` array; the 300th type's cast back to another type (a different
// specialization, or the `int` at the same address) is still diagnosed,
// with no fallback past a fixed number of types (spec/oven.md
// "Construction").
//@ expect: compile-fail
//@ strict-diags
//@ std: c++26
//@ diag: 43:15 error "static assertion expression is not an integral constant expression"
//@ diag: 37:16 note "cast from 'void *' is not allowed in a constant expression because the pointed object type 'S<300>' is not similar to the target type 'S<299>'"
//@ diag: 43:15 note "in call to 'read<300, S<299>>()'"
//@ diag: 44:15 error "static assertion expression is not an integral constant expression"
//@ diag: 37:16 note "cast from 'void *' is not allowed in a constant expression because the pointed object type 'S<300>' is not similar to the target type 'int'"
//@ diag: 44:15 note "in call to 'read<300, int>()'"
template <int N> struct S { int v = N; };
template <int N> struct Store { S<N> s; Store<N - 1> rest; };
template <> struct Store<0> {};

// Stores `&st.s` of every level as a `void *`, `S<1>` first: `S<300>` is
// the 300th distinct designated type stored to memory.
template <int N> constexpr void fill(void **out, Store<N> &st) {
  if constexpr (N > 0) {
    fill<N - 1>(out, st.rest);
    out[N - 1] = &st.s;
  }
}

template <class T> constexpr int value(T *p) { return p->v; }
constexpr int value(int *p) { return *p; }

template <int K, class T> constexpr int read() {
  Store<300> st{};
  void *out[300] = {};
  fill<300>(out, st);
  return value(static_cast<T *>(out[K - 1]));
}

static_assert(read<1, S<1>>() == 1);
static_assert(read<255, S<255>>() == 255);
static_assert(read<300, S<300>>() == 300);
static_assert(read<300, S<299>>() == 300);
static_assert(read<300, int>() == 300);
