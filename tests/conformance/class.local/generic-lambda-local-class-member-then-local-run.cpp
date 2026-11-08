// [class.local]/1, [dcl.fct]/22, [expr.prim.lambda.general]/6: the
// invented template head of a generic lambda's `auto` parameter stays
// open for the whole lambda body. A member declaration of a local class
// in that body ends only the member's own declaration, so the locals
// declared after the class are still declarations of the call operator
// template, instantiated with each specialization (WP-789).
//@ expect: run
//@ std: c++20
//@ stdout: 1 2 5 7 3
extern "C" int printf(const char *, ...);

auto with_dtor = [](auto x) {
  struct guard {
    int *p;
    ~guard() { ++*p; }
  };
  int orig = 0;
  {
    guard g{&orig};
  }
  return orig + x - 1;
};

auto with_members = [](auto x) {
  struct acc {
    int v;
    int get() const { return v; }
    static int twice(int a) { return a + a; }
  };
  int total = acc{x}.get();
  total += acc::twice(x);
  return total;
};

auto with_nested = [](auto x) {
  struct outer {
    struct inner {
      int bump(int y) { return y + 1; }
    };
    int n = 0;
  };
  int r = typename outer::inner{}.bump(x);
  return r;
};

int main() {
  printf("%d %d %d %d %d\n", with_dtor(1), with_dtor(2), with_members(1) + 2,
         with_nested(6), with_members(1));
}
