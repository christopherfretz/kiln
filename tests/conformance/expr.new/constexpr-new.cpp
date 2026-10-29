// [expr.new], [expr.delete], [expr.const]/5.20: scalar, array,
// multidimensional and aggregate new-expressions with every initializer form,
// freed with the matching delete, are constant; deleting a null pointer does
// nothing.
// Mirrors crates/kiln_oven/tests/new_delete.rs
// `expr_new_scalar_and_array_roundtrip`.
struct S {
  int a, b;
};
union U {
  int i;
  float f;
};

constexpr int scalar(int n) {
  int *p = new int(n);
  const int *c = new const int(n + 1);
  int *z = new int();
  int *d = new int;
  *d = 5;
  int r = *p + *c + *z + *d;
  delete p;
  delete c;
  delete z;
  delete d;
  return r;
}
static_assert(scalar(1) == 8);

constexpr int arrays(int n) {
  int *a = new int[n]{1, 2};
  int *z = new int[n]();
  S *s = new S[n]{{1, 2}};
  int(*m)[2] = new int[n][2]{{1, 2}, {3, 4}};
  int **pp = new int *[n] {};
  int r = 0;
  for (int i = 0; i < n; ++i) r += a[i] + z[i] + s[i].a + s[i].b + m[i][0] + m[i][1];
  bool null = pp[n - 1] == nullptr;
  delete[] a;
  delete[] z;
  delete[] s;
  delete[] m;
  delete[] pp;
  return null ? r : -1;
}
static_assert(arrays(4) == 16);
static_assert(arrays(2) == 16);

constexpr int aggregate() {
  S *s = new S{3, 4};
  U *u = new U{};
  int r = s->a * s->b + u->i;
  u->f = 1.0f;
  delete s;
  delete u;
  return r;
}
static_assert(aggregate() == 12);

constexpr int distinct() {
  int *p = new int(1);
  int *q = new int(1);
  bool r = p != q;
  delete p;
  delete q;
  int *n = nullptr;
  delete n;
  delete[] n;
  return r;
}
static_assert(distinct());
