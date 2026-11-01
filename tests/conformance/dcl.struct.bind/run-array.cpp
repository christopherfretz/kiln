//@ expect: run
//@ stdout: 1 2 7 31 ab0
// [dcl.struct.bind]/3 (C++26 /6) at run time: `auto` copies the array
// (element by element), the reference forms alias it.
extern "C" int printf(const char *, ...);
int arr[2] = {1, 2};
int main() {
  auto [a, b] = arr;
  auto &[c, d] = arr;
  c = 3;
  d = 4;
  int m[2][2] = {{1, 2}, {3, 4}};
  auto &[r0, r1] = m;
  r1[1] = 30;
  const auto &[x, y, z] = "ab";
  printf("%d %d %d %d %c%c%d\n", a, b, c + d, r1[1] + r0[0], x, y, z);
}
