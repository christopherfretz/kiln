// [expr.unary.op]: unary & yields a pointer to its operand, unary * the
// lvalue it points to; ! and ~ and unary minus on integers.
//@ expect: run
//@ stdout: 5 9 9 1 0 -6 -5
extern "C" int printf(const char*, ...);

int main() {
  int v = 5;
  int* p = &v;
  int** pp = &p;
  int before = *p;
  **pp = 9;
  printf("%d %d %d %d %d %d %d\n", before, v, *&v, !0, !v, ~v + 4, -before);
}
