// [dcl.fct.def.coroutine]/13: each parameter is copied (move-constructed
// from an xvalue) into the coroutine state before the promise is
// constructed; the body uses the copies, which outlive the caller's
// arguments and are destroyed with the coroutine state. A reference
// parameter is not copied.
//@ expect: run
//@ std: c++20
//@ stdout: move 1
//@ stdout: dtor 0
//@ stdout: started
//@ stdout: body 1 7
//@ stdout: dtor 1
//@ stdout: done 8
#include "Inputs/coro.h"

struct P {
  int v;
  explicit P(int x) : v(x) {}
  P(P&& o) : v(o.v) {
    printf("move %d\n", v);
    o.v = 0;
  }
  ~P() { printf("dtor %d\n", v); }
};

gen use(P p, int& r) {
  co_yield 0;
  printf("body %d %d\n", p.v, r);
  ++r;
}

int main() {
  int r = 7;
  gen g = [&] {
    gen x = use(P(1), r);
    printf("started\n");
    return x;
  }();
  g.next();
  g.next();
  g.h.destroy();
  g.h = {};
  printf("done %d\n", r);
}
