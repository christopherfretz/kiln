// Per-specialization entries (spec/ast.md §18.9.1): the table does not
// depend on the order of instantiation. This file and inst-order-a.cpp
// are the same program but for the order of the two calls in `run`; their
// goldens differ only in the lines of `run` (each entry's `spec=` lines
// print sorted, so `measure<Alpha>`'s and `measure<Beta>`'s lines appear
// in the same order whichever was instantiated first).
//@ written-refs: inst-order-b.refs
struct Alpha {
  int size() const { return 1; }
  using kind = int;
};
struct Beta {
  int size() const { return 2; }
  using kind = long;
};

template <class T> int measure(const T &t) {
  typename T::kind k = 0;
  return t.size() + static_cast<int>(k);
}

int run() {
  int b = measure(Beta{});
  int a = measure(Alpha{});
  return a + b;
}
