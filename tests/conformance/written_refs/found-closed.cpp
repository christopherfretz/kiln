// Closed found sets (spec/ast.md §18.12): a parenthesized name and a
// qualified name are not subject to argument-dependent lookup
// ([basic.lookup.argdep]/1, [temp.dep.candidate]), so every
// instantiation selects from the set (`def=n` without `+adl`). A
// block-scope function declaration suppresses argument-dependent lookup
// too: the call is resolved at the definition and the entry has its
// entity, not a set.
//@ written-refs: found-closed.refs
namespace N {
void h(int);
void h(long);
} // namespace N
void f(int);
void f(long);
template <class T> void closed(T t) {
  (f)(t);
  ::f(t);
  N::h(t);
}
template <class T> void block(T t) {
  void f(int);
  f(t);
}
void use() {
  closed(1);
  block(1);
}
