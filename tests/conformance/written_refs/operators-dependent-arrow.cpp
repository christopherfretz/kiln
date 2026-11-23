// Dependent `->` chains (spec/ast.md §18.13.6): `t->m` with a
// type-dependent `t` is a DEPENDENT entry at `->` with no found set (an
// `operator->` is a member, [over.ref]/1). A specialization whose `t` is a
// class applies `operator->` repeatedly until the result is a pointer
// ([over.match.oper]/2, [over.ref]/1): one row per `operator->` applied,
// in application order, each `how=member`. A specialization whose `t` is
// a pointer has one `(builtin)` row ([expr.ref]/2). A template never
// instantiated and a discarded `if constexpr` branch ([stmt.if]/2) have
// the entry and no row.
//@ written-refs: operators-dependent-arrow.refs
struct S {
  int v;
};
struct P {
  S *operator->();
};
struct Q {
  P operator->();
};

template <class T> int get(T t) { return t->v; }
template <class T> int never(T t) { return t->v; }
template <class T> int discard(T t) {
  if constexpr (sizeof(T) > 1000)
    return t->v;
  else
    return 0;
}
template <class T> int call(T t) { return t.operator->()->v; }

int run() {
  S s{};
  return get(Q{}) + get(P{}) + get(&s) + discard(&s) + call(Q{});
}
