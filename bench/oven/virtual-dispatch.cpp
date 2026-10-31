//@ args: -fconstexpr-steps=100000000
// Oven virtual dispatch (WP-077): 100000 virtual calls through base
// pointers into a 3-level hierarchy (Shape > Poly > Rect/Tri, with a
// second base so half the overriders need a `this` adjustment), over an
// array of 64 objects of 4 dynamic types. Exercises Op::VCall's
// dynamic-type read, the resolved-call cache and adjusted `this`.

struct Tag {
  int tag = 7;
  constexpr virtual int weight() const { return tag; }
};

struct Shape {
  int id = 0;
  constexpr virtual ~Shape() {}
  constexpr virtual int area() const { return 0; }
  constexpr virtual int sides() const { return 0; }
};

struct Poly : Shape {
  int n = 0;
  constexpr int sides() const override { return n; }
};

struct Rect : Poly {
  int w = 2, h = 3;
  constexpr int area() const override { return w * h; }
};

struct Tri : Tag, Poly {
  int b = 4, h = 5;
  constexpr int area() const override { return b * h / 2 + weight(); }
  constexpr int sides() const override { return 3; }
};

struct Square final : Rect {
  constexpr int area() const override { return w * w; }
};

constexpr long run(int calls) {
  Rect r[16];
  Tri t[16];
  Square s[16];
  Poly p[16];
  Shape *all[64];
  for (int i = 0; i < 16; ++i) {
    r[i].n = 4;
    s[i].n = 4;
    p[i].n = i;
    all[4 * i] = &r[i];
    all[4 * i + 1] = &t[i];
    all[4 * i + 2] = &s[i];
    all[4 * i + 3] = &p[i];
  }
  long sum = 0;
  for (int i = 0; i < calls / 2; ++i) {
    const Shape *x = all[i & 63];
    sum += x->area() + x->sides();
  }
  return sum;
}

static_assert(run(100000) == 568726);
