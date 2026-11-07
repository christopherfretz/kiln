// [dcl.init.aggr]/5.4, [class.mem.general]/12, [expr.prim.this]/2: a
// default member initializer used by an aggregate initialization names,
// with `this` (written or implicit), the aggregate being initialized:
// not the `this` of the enclosing member function, and also in a
// non-member function. A sender's operation state that registers
// `*this` with a stop callback relies on it (WP-789).
//@ expect: run
//@ std: c++20
//@ stdout: 1 1 2 | 1 1 11 | 1 1 21 | 1 1 31 | 1 1 41 7
extern "C" int printf(const char *, ...);

struct op;
struct back {
  op &o;
};
struct pinned {
  int *p;
  pinned(int *q) : p(q) {}
  pinned(const pinned &) = delete;
};
struct op {
  int r;
  int tok = r + 1;
  back b{*this};
  pinned cb = pinned(&tok);
  void show(const char *sep) const {
    printf("%d %d %d%s", &b.o == this, cb.p == &tok, tok, sep);
  }
};

struct sender {
  int base;
  op connect(int r) const { return op{base + r}; }
};

struct holder {
  op o;
  int extra = o.r - o.tok + 8;
  template <class F> holder(F f) : o(f()) {}
};

struct outer {
  int k;
  op inner{k};
};

op make(int r) { return op{r}; }

int main() {
  op a = make(1);
  a.show(" | ");
  op b = sender{10}.connect(0);
  b.show(" | ");
  holder h([] { return sender{20}.connect(0); });
  h.o.show(" | ");
  op *p = new op(sender{30}.connect(0));
  p->show(" | ");
  delete p;
  outer o{40};
  o.inner.show("");
  printf(" %d\n", h.extra);
}
