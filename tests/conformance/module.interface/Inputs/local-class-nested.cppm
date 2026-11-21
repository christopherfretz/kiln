export module lcnest;
// A member class of a local class, and a local class of a local class's
// member function.
export inline int lcnest_member(int x) {
  struct O {
    struct I {
      int a;
      int get() const { return a * 2; }
    };
    I i;
    int get() const { return i.get() + 1; }
  };
  return O{{x}}.get();
}
export inline int lcnest_inner(int x) {
  struct Outer {
    int a;
    int run() const {
      struct Inner {
        int b;
        int twice() const { return b + b; }
      };
      return Inner{a}.twice() + 1;
    }
  };
  return Outer{x}.run();
}
