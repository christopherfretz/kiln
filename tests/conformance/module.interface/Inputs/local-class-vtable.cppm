export module lcvt;
// Local classes whose vtables nothing in this unit uses: J's key function
// g is defined here, I has none (its destructor is defaulted).
export inline int lcvt_heap(int x) {
  struct I {
    virtual int g(int) const = 0;
    virtual ~I() = default;
  };
  struct J : I {
    int g(int y) const override { return y * 3; }
  };
  I *p = new J;
  int r = p->g(x);
  delete p;
  return r;
}
