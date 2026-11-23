// Per-specialization entries (spec/ast.md §18.9.3): an instantiation that
// fails with an error ([temp.res.general]/6 makes the program ill-formed;
// not a substitution failure, [temp.deduct.general]/8) keeps what it
// resolved: `both<Good>` resolves every name, `both<Bad>` resolves the
// two `size` calls and records `count`, which its class does not declare,
// with no entity (`(unresolved)`).
//@ expect: compile-fail
//@ written-refs: inst-error.refs
//@ diag: 20:13 error "no member named 'count' in 'Bad'"
struct Good {
  int size() const { return 1; }
  int count() const { return 2; }
};
struct Bad {
  int size() const { return 1; }
};

template <class T> int both(const T &t) {
  int a = t.size();
  int b = t.count();
  int c = t.size();
  return a + b + c;
}

int run() { return both(Good{}) + both(Bad{}); }
