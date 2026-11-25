// A using-declaration through a lattice of virtual bases (spec/sema.md §3,
// "Class member lookup cost"; WP-3404): La<k>, Lb<k> : virtual La<k-1>,
// virtual Lb<k-1>, with f declared in La<0> only, so La<N> has 2^N paths
// to La<0> but 2N+1 classes. Measures the class member lookup of a
// using-declarator's name and of member accesses, and the access checks
// of the members found. Reduced from real code. Before WP-3404 kiln searched
// every path (depth 10: 26 s debug); clang 19 is exponential past depth 20
// (depth 40: over 2 minutes). The depth is LATTICE_DEPTH (default 20).
// budget-time: about 3x kiln's median when recorded (31 ms, release,
// 2026-11-24), loose for a loaded host; the search per path took 28 s.
//@ budget-time: 100

#ifndef LATTICE_DEPTH
#define LATTICE_DEPTH 20
#endif

template <int N> struct La;
template <int N> struct Lb;
template <> struct La<0> {
  void f();
  void g(int);
};
template <> struct Lb<0> : virtual La<0> {};
template <int N> struct La : virtual La<N - 1>, virtual Lb<N - 1> {};
template <int N> struct Lb : virtual La<N - 1>, virtual Lb<N - 1> {};

struct D : La<LATTICE_DEPTH> {
  void f(int);
  using La<LATTICE_DEPTH>::f;
  using La<LATTICE_DEPTH>::g;
};

void h(D &d, Lb<LATTICE_DEPTH> &e) {
  d.f(1);
  d.f();
  d.g(2);
  e.f();
  e.La<0>::g(3);
}
