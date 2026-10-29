// [expr.new]: array new with a constant and a run-time bound; a braced
// initializer fills the leading elements and value-initializes the tail;
// multidimensional new yields a pointer to the first row.
//@ expect: run
//@ stdout: 1 2 3 0 0 | 0 0 0 0 0 0 | 10 11 12 13 | 6
extern "C" int printf(const char*, ...);

int count(int n) { return n; }

int main() {
  int* a = new int[5]{1, 2, 3};
  for (int i = 0; i < 5; ++i) printf("%d ", a[i]);
  printf("| ");
  int n = count(6);
  short* z = new short[n]();
  for (int i = 0; i < n; ++i) printf("%d ", z[i]);
  printf("| ");
  int (*m)[2] = new int[n][2];
  for (int i = 0; i < n; ++i) {
    m[i][0] = 10 + i;
    m[i][1] = i;
  }
  for (int i = 0; i < 4; ++i) printf("%d ", m[i][0]);
  printf("| %d\n", n);
  delete[] m;
  delete[] z;
  delete[] a;
}
