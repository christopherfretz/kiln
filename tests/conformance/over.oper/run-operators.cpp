// [over.oper], [over.ref]: overloaded operators called at run time.
//@ expect: run
//@ stdout: 5 8 -5 -8 13 -5
//@ stdout: 1 1
extern "C" int printf(const char *, ...);
struct V {
  int x, y;
  V operator+(const V &o) const { return {x + o.x, y + o.y}; }
  V &operator+=(const V &o) { x += o.x; y += o.y; return *this; }
  bool operator==(const V &o) const { return x == o.x && y == o.y; }
  int operator[](int i) const { return i ? y : x; }
  int operator()(int a, int b) const { return a * x + b * y; }
};
V operator-(V a) { return {-a.x, -a.y}; }
struct P { V *p; V *operator->() const { return p; } };
int main() {
  V a{1, 2}, b{3, 4};
  V c = a + b;
  c += a;
  V d = -c;
  P p;
  p.p = &d;
  printf("%d %d %d %d %d %d\n", c.x, c.y, d[0], d[1], c(1, 1), p->x);
  printf("%d %d\n", a == a, a != b);
  return 0;
}
