// [namespace.udecl]/3, /14: a three-level chain of class templates, each
// re-exporting the base `operator=` set with a using-declarator (one
// through a nested-name-specifier naming the grandparent via a typedef
// of the parent, `ReadOnlyMapBase::Base`) and the most derived one
// declaring `operator=` overloads with the same signatures, which hide
// the introduced ones. WP-612.
//@ expect: run
//@ strict-diags
template <class D> struct DenseBase { int v; };
template <class D> struct MatrixBase : DenseBase<D> {
  typedef DenseBase<D> Base;
  D& operator=(const MatrixBase& o) { this->v = o.v; return static_cast<D&>(*this); }
  template <class O> D& operator=(const DenseBase<O>& o) { this->v = o.v; return static_cast<D&>(*this); }
};
template <class D, int Level> struct MapBase;
template <class D> struct MapBase<D, 0> : MatrixBase<D> {
  typedef MatrixBase<D> Base;
  using Base::operator=;
};
template <class D> struct MapBase<D, 1> : MapBase<D, 0> {
  typedef MapBase<D, 0> ReadOnlyMapBase;
  typedef MapBase<D, 0> Base;
  D& operator=(const MapBase& o) { ReadOnlyMapBase::Base::operator=(o); return static_cast<D&>(*this); }
  using ReadOnlyMapBase::Base::operator=;
};
template <class T> struct Map : MapBase<Map<T>, 1> {
  typedef MapBase<Map<T>, 1> Base;
  using Base::operator=;
  Map& operator=(const Map& o) { Base::operator=(o); return *this; }
  template <class O> Map& operator=(const DenseBase<O>& o) { Base::operator=(o); this->v *= 2; return *this; }
};
template <class T> struct Block : MatrixBase<Block<T>> {};
int main() {
  Map<double> m, n;
  Block<double> b;
  b.v = 3;
  m = b;
  if (m.v != 6) return 1;
  n = m;
  if (n.v != 6) return 2;
  return 0;
}
