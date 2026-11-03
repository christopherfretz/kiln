//@ mode: emit-llvm
// Deep virtual lattices through codegen (spec/codegen.md "Vtables and
// VTTs of virtual lattices"; WP-414): LATTICE_DEPTH (default 12) virtual
// diamonds, D<N> : B<N>, C<N> with B<N>, C<N> : virtual D<N-1>, so each
// class has 2^N paths to D<0> but 3N+1 subobjects. Emitting D<N>'s
// constructors and destructors emits the vtable group, VTT and
// construction vtables of every class of the lattice, and their RTTI.
// The output is cubic in the depth; clang's vtable marking and RTTI flag
// walk are exponential in it (about 1 s here, 12 s at depth 16), kiln
// visits each class once. virtual-lattice.cpp measures sema alone.

#ifndef LATTICE_DEPTH
#define LATTICE_DEPTH 12
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
