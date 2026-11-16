//@ expect: run
//@ strict-diags
//@ args: -std=gnu++20
//@ stdout: 12 1 16 2 20 3 24 4
// An explicit conversion to a variably modified type (GNU extension)
// evaluates the type's array bounds where the conversion is evaluated
// (clang `EmitExplicitCastExprType`), also when the cast is only the
// operand of `*` under `sizeof` and for a C++ cast form. Found running
// GCC's C++ testsuite.
extern "C" int printf(const char *, ...);

int calls;
int next() { return 3 + calls++; }

unsigned long null_deref() { return sizeof(*(int (*)[next()])0); }
unsigned long from_pointer(int *p) { return sizeof(*(int (*)[next()])p); }
unsigned long stored(int *p) {
  auto q = (int (*)[next()])p;
  return sizeof *q;
}
unsigned long cxx_cast(int *p) {
  typedef int A[next()];
  return sizeof *reinterpret_cast<A *>(p);
}

int main() {
  int buf[8] = {};
  unsigned long a = null_deref();
  int c1 = calls;
  unsigned long b = from_pointer(buf);
  int c2 = calls;
  unsigned long c = stored(buf);
  int c3 = calls;
  unsigned long d = cxx_cast(buf);
  printf("%lu %d %lu %d %lu %d %lu %d\n", a, c1, b, c2, c, c3, d, calls);
}
