// [namespace.udecl]/14, [temp.over.link]/6: in an implicitly instantiated
// class template specialization, a member function template hides the
// base member function template a using-declarator introduced when they
// have the same name, parameter-type-list, cv- and ref-qualifiers and
// equivalent template heads; the base one is then not a candidate.
// Without the hiding `m = b` is ambiguous. WP-612.
//@ expect: run
//@ strict-diags
template <class D> struct DenseBase { int v; };
template <class D> struct MatrixBase : DenseBase<D> {
  template <class O> D& operator=(const DenseBase<O>& o) { this->v = o.v; return static_cast<D&>(*this); }
};
template <class T> struct Map : MatrixBase<Map<T>> {
  typedef MatrixBase<Map<T>> Base;
  using Base::operator=;
  template <class O> Map& operator=(const DenseBase<O>& o) { Base::operator=(o); this->v += 1; return *this; }
};
template <class T> struct Block : MatrixBase<Block<T>> {};
int main() { Map<double> m; Block<double> b; b.v = 3; m = b; return m.v - 4; }
