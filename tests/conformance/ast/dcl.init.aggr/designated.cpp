//@ expect: ast
struct Pt { int x; int y; int z; };
int dz() {
  Pt p = {.x = 1, .z = 3};
  Pt q{.y = 2};
  return p.z + q.y;
}
