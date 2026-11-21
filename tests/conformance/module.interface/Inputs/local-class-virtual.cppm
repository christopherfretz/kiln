export module lcvirt;
// Local classes with virtual members: B's key function f is not inline,
// so its vtable is the module unit's; D's destructor is implicit.
export inline int lcvirt_call(int x) {
  struct B {
    virtual int f() const { return 1; }
    virtual ~B() {}
  };
  struct D : B {
    int v;
    D(int v) : v(v) {}
    int f() const override { return v; }
  };
  D d(x);
  const B &b = d;
  B plain;
  return b.f() + plain.f();
}
