// [expr.ref]: E1.E2 and E1->E2 designate the named member of the object;
// members of nested class objects and arrays of class objects.
//@ expect: run
//@ stdout: 3 4 | 7 1 2 | 36
extern "C" int printf(const char*, ...);

struct Point {
  int x, y;
};
struct Seg {
  Point a, b;
  int tag;
};

void shift(Seg* s, int d) {
  s->a.x += d;
  s->b.y += d;
}

int main() {
  Seg s = {{1, 2}, {3, 4}, 7};
  Seg* p = &s;
  printf("%d %d | ", p->b.x, s.b.y);
  shift(p, 0);
  printf("%d %d %d | ", (*p).tag, p->a.x, s.a.y);
  Point pts[3] = {{1, 2}, {3, 4}, {5, 6}};
  int sum = 0;
  for (int i = 0; i < 3; ++i) sum += pts[i].x * pts[i].y - (&pts[i])->x;
  printf("%d\n", sum + 1);
}
