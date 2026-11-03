// [class.temporary]/4, [expr.cond]/1, [expr.log.or]/1, [stmt.expr]/1: a
// full-expression's temporaries are destroyed at its end; one created in
// a conditional arm is destroyed under the arm's condition (the
// control-flow graph, spec/sema.md §7.5.1).
//@ expect: cfg
//@ cfg-diverges: nested clang leaves the destructor decision of a temporary in a nested operand of a `?:` arm unreachable (CFG.cpp VisitConditionalOperatorForTemporaryDtors drops the true arm without its own temporary)
struct T {
  T();
  T(int);
  ~T();
  int get() const;
};
int h(int);

int simple() {
  return T().get();
}

int arm(int x) {
  return x ? T(1).get() : 2;
}

int value(int x, int y) {
  int v = x ? y : h(y);
  return v ? h(1) : h(2);
}

bool logic(int x) {
  bool b = x || T().get();
  return b && h(x);
}

void extended() {
  const T &r = T();
  h(r.get());
}

int both(int x) {
  return x ? T(1).get() : T(2).get();
}

int nested(int x, int y) {
  return x ? (y || T(1).get()) : 0;
}

int seq(int x) {
  return T().get() + (x ? T(2).get() : 3);
}

int known() {
  return true ? T(1).get() : T(2).get();
}

struct N {
  N();
  [[noreturn]] ~N();
  int get() const;
};

int noret(int x) {
  if (x)
    return N().get();
  return 0;
}
