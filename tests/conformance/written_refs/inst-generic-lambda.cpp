// Per-specialization entries (spec/ast.md §18.9.2): a generic lambda's
// call operator is a member function template of the closure type
// ([expr.prim.lambda.closure]/3), so `x.size()` is resolved by each
// specialization of the call operator (`spec=` names it), while the
// enclosing function template's own dependent name `t.size()` is resolved
// by the function template specialization.
//@ written-refs: inst-generic-lambda.refs
struct One {
  int size() const { return 1; }
};
struct Two {
  int size() const { return 2; }
};

template <class T> int outer(const T &t) {
  auto f = [](const auto &x) { return x.size(); };
  return f(t) + f(Two{}) + t.size();
}

int run() { return outer(One{}); }
