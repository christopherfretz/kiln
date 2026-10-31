//@ expect: run
//@ stdout: 1 2 11 12 5 6 7
// [dcl.struct.bind]/5 (C++26 /7) at run time (argc is 1): bindings to data members,
// including inherited ones, a bit-field and a temporary whose lifetime is
// extended by the holding reference.
extern "C" int printf(const char *, ...);
struct B { int x; int y; };
struct D : B {};
struct BF { int a : 4; int b : 4; };
D make(int v) { return D{{v, v + 1}}; }
int main(int argc, char **) {
  D d{{argc, 2}};
  auto [x, y] = d;
  auto &[rx, ry] = d;
  rx += 10;
  ry += 10;
  const auto &[tx, ty] = make(argc + 4);
  BF bf{argc + 2, 4};
  auto &[ba, bb] = bf;
  ba = bb + 3;
  printf("%d %d %d %d %d %d %d\n", x, y, d.x, d.y, tx, ty, bf.a);
}
