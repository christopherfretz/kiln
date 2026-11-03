// [class.virtual]/2 (final overriders), [class.member.lookup], [conv.ptr]/3,
// [basic.def.odr]/8 (a used vtable odr-uses its virtual members): a lattice
// of 32 virtual diamonds, D<N> : B<N>, C<N> with B<N>, C<N> : virtual
// D<N-1>. Every class has 2^N paths to D<0> but only 3N+1 subobjects. The
// vtable marking (clang `MarkVirtualMembersReferenced` recurses into each
// base with virtual bases), the virtual-base collection, member lookup of a
// name declared nowhere (`operator delete` for the virtual destructors) and
// the derived-to-base path search each walk a class once, not once per
// path (WP-400): before, depth 16 took 25 s and depth 32 would not finish
// within the compile limit; clang 22 is exponential here too (depth 14:
// 2.5 s, 16: 9.4 s), so it is skipped. bench/frontend/virtual-lattice.cpp
// measures depth 14 against clang. Sema only: the vtables and VTTs of such
// a lattice are not yet emitted in polynomial time (codegen).
//@ skip(clang): clang's vtable marking is exponential in the lattice depth
//@ expect: compile-ok
//@ strict-diags
//@ mem<=512
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
  D<32> d;
  D<0> &r = d;
  D<16> &m = d;
  return r.f() + r.g() + (m.f() - 32);
}
