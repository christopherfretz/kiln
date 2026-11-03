// Deep virtual lattices (spec/sema-class.md §10, "Virtual lattices"; WP-400):
// 14 virtual diamonds, D<N> : B<N>, C<N> with B<N>, C<N> : virtual D<N-1>,
// so each class has 2^N paths to D<0> but 3N+1 subobjects. Measures the
// final-overrider computation, the vtable marking of every class's virtual
// members, and member lookup and base-path searches through virtual bases.
// clang's `MarkVirtualMembersReferenced` recursion is exponential in the
// depth (about 2.5 s here); kiln walks each class once. The depth is
// LATTICE_DEPTH (default 14; e.g. `args: -DLATTICE_DEPTH=16` or by hand).
// virtual-lattice-emit.cpp measures the vtables and VTTs of the same shape.

#ifndef LATTICE_DEPTH
#define LATTICE_DEPTH 14
#endif

template <int N> struct D;
template <> struct D<0> {
  virtual int f() const { return 0; }
  virtual int g() const { return 0; }
  virtual ~D() {}
};
template <int N> struct B : virtual D<N - 1> {
  int f() const override { return N; }
};
template <int N> struct C : virtual D<N - 1> {
  int g() const override { return N; }
};
template <int N> struct D : B<N>, C<N> {
  int f() const override { return N; }
  int g() const override { return -N; }
};
int main() {
  D<LATTICE_DEPTH> d;
  D<0> &r = d;
  return r.f() + r.g();
}
