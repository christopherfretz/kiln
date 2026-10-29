// Scalar compound literals: a prvalue of the written type whose
// InitListExpr has the cv-unqualified type; postfix operators apply.
enum E { A, B };
int f(int a) {
  int x = (int){4};
  const long c = (const long){a};
  int z = (int){};
  E e = (E){B};
  int *p = (int *){nullptr};
  int s = (int){1} + (const int){2} * 3;
  (void)(int){{5}};
  return x + c + z + e + (p == nullptr) + s;
}
