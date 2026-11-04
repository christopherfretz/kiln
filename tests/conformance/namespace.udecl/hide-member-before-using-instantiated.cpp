// [namespace.udecl]/14: in an instantiated class template specialization
// the member function (template) declared BEFORE the using-declarator
// hides the base one the declarator would introduce (the introduced
// declaration is dropped when the using-declaration is instantiated).
// WP-612.
//@ expect: run
//@ strict-diags
template <class D> struct DenseBase { int v; };
template <class D> struct MatrixBase : DenseBase<D> {
  template <class O> D& operator=(const DenseBase<O>& o) { this->v = 1; return static_cast<D&>(*this); }
  D& operator=(int) { this->v = 1; return static_cast<D&>(*this); }
};
template <class T> struct Map : MatrixBase<Map<T>> {
  template <class O> Map& operator=(const DenseBase<O>&) { this->v = 2; return *this; }
  Map& operator=(int) { this->v = 2; return *this; }
  using MatrixBase<Map<T>>::operator=;
};
template <class D> struct Block : MatrixBase<Block<D>> {};
int main() {
  Map<int> m;
  Block<int> b;
  m = b;
  if (m.v != 2) return 1;
  m.v = 0;
  m = 3;
  if (m.v != 2) return 2;
  return 0;
}
