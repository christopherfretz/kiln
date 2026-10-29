// [expr.delete]: deleting a null pointer has no effect; delete and delete[]
// accept pointers to complete object types including pointer-to-array.
//@ expect: run
//@ stdout: done 3
extern "C" int printf(const char*, ...);

int main() {
  int* p = nullptr;
  delete p;
  double* q = nullptr;
  delete[] q;
  int (*m)[3] = new int[2][3]{{1, 2, 3}, {4, 5, 6}};
  int v = m[0][2];
  delete[] m;
  struct S { int a; };
  S* s = new S{3};
  printf("done %d\n", s->a + v - 3);
  delete s;
}
