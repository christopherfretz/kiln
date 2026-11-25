// Substitution keys (spec/ast.md §18.14.10): a construction in a partial
// specialization's argument list. Substituting `decltype(void(T(1)))`
// with `T` = `K` ([temp.spec.partial.match]/2, [temp.deduct]/5) selects
// the constructor `K::K(int)` ([expr.type.conv]/2, [over.match.ctor]/1);
// no written name selects it, so the key has no row (`rows=0`), and the
// key's `root[1]` holds the construction, whose constructor a reader
// finds by walking it.
//@ written-refs: subst-partial-ctor.refs
//@ args: -fkiln-written-ref-keys
//@ std: c++26
struct K {
  K(int);
};
template <class T, class = void> struct C {
  static const int v = 0;
};
template <class T> struct C<T, decltype(void(T(1)))> {
  static const int v = 1;
};
C<K> c;
static_assert(C<K>::v == 1);
