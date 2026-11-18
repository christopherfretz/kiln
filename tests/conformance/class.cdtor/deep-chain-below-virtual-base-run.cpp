// [class.cdtor]/4: a virtual function called, directly or through a
// reference to a base, from a constructor or destructor resolves to the
// final overrider in the class of that constructor or destructor. A
// chain 192 levels deep with a virtual base at its root has 192
// construction vtables in its VTT (Itanium C++ ABI §2.6), and each class
// of it its own VTT: the calls through the virtual base during
// construction and destruction at several levels, and after construction,
// check the construction vtables (WP-1650: the chain's construction
// groups are computed in time proportional to what they hold). 192 is
// the depth a debug build of kiln compiles in under 5 s (the vtables are
// no longer the cost: the IR is quadratic in the depth).
//@ expect: run
//@ stdout: ok
extern "C" int printf(const char *, ...);

constexpr int Depth = 192;

struct Root {
  virtual ~Root() {}
  virtual int f() const { return -1; }
};

int probe(const Root &r) { return r.f(); }

int built[Depth + 1];
int destroyed[Depth + 1];

constexpr bool checked(int n) { return n % 64 == 0 || n == 1 || n == Depth - 1; }

template <int N> struct Level : Level<N - 1> {
  Level() {
    if (checked(N))
      built[N] = probe(*this);
  }
  ~Level() override {
    if (checked(N))
      destroyed[N] = probe(*this);
  }
  int f() const override { return N; }
};

template <> struct Level<0> : virtual Root {
  Level() { built[0] = probe(*this); }
  ~Level() override { destroyed[0] = probe(*this); }
  int f() const override { return 0; }
};

int main() {
  for (int i = 0; i <= Depth; ++i)
    built[i] = destroyed[i] = -2;
  int after = -2;
  {
    Root *r = new Level<Depth>;
    after = r->f();
    delete r;
  }
  int bad = after != Depth;
  for (int i = 0; i <= Depth; ++i) {
    int want = i == 0 || checked(i) ? i : -2;
    bad += built[i] != want;
    bad += destroyed[i] != want;
  }
  printf(bad ? "bad %d\n" : "ok\n", bad);
  return bad;
}
