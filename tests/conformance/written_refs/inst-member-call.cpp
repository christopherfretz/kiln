// Per-specialization entries (spec/ast.md §18.9, stage 4): a member call
// on a dependent object ([temp.dep.expr]/5, [expr.ref]) is looked up at
// instantiation ([temp.res.general]/1). The pattern's `size`, `get` and
// `count` stay one entry each with no entity; each specialization adds a
// `spec=` line naming the member its class declares, the selected
// overload for an overloaded member ([over.match.funcs]).
//@ written-refs: inst-member-call.refs
struct Vec {
  int size() const { return 1; }
  int get(int) const { return 2; }
  int get(long) const { return 3; }
  int count;
};
struct List {
  int size() const { return 4; }
  int get(int) const { return 5; }
  int count;
};

template <class T> int measure(const T &t) {
  return t.size() + t.get(1) + t.count;
}

template <class T> int measure_ptr(const T *p) { return p->size(); }

int run() {
  Vec v{};
  List l{};
  return measure(v) + measure(l) + measure_ptr(&v);
}
