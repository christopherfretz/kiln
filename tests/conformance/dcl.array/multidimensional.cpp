// [dcl.array]: a multidimensional array is an array of arrays laid out in
// row-major order.
//@ expect: run
//@ stdout: 0 1 2 10 11 12
//@ stdout: 12 6 3
extern "C" int printf(const char*, ...);

int main() {
  int m[2][3];
  for (int i = 0; i < 2; ++i)
    for (int j = 0; j < 3; ++j) m[i][j] = i * 10 + j;
  int* flat = &m[0][0];
  for (int k = 0; k < 6; ++k) printf(k == 5 ? "%d\n" : "%d ", flat[k]);
  printf("%zu %zu %zu\n", sizeof m / sizeof(int) * 2, sizeof m / sizeof m[0][0],
         sizeof m[0] / sizeof m[0][0]);
}
