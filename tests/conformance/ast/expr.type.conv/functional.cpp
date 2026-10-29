// [expr.type.conv]: T(e), T(), T{...} and T{} for scalar T.
typedef unsigned U;
enum E { A, B };
int f(double d, int i) {
  int a = int(d);
  int b = int();
  int c = int{i};
  int z = int{};
  U u = U(i);
  E e = E(1);
  long l = long{3};
  double *p = (double *)nullptr;
  (void)p;
  return a + b + c + z + int(u) + e + int(l);
}
