// A conditional of GNU vector elements is a prvalue (clang 22; see
// p5-vector-element-prvalue-errors.cpp): it reads the selected element,
// binds a const reference to a temporary holding it, and converts like
// any int prvalue (the const reference keeps the value it read). clang
// 22 and GCC 16.1 agree on these. WP-1700.
//@ expect: run
//@ strict-diags
//@ stdout: 1 6 | 1 4 1 | 6 8 9 | end
extern "C" int printf(const char *, ...);
typedef int v4 __attribute__((vector_size(16)));

__attribute__((noinline)) void g(int c) {
  v4 v = {1, 2, 3, 4}, w = {5, 6, 7, 8};
  int s = 9;
  int y = c ? v[0] : w[1];
  const int &cr = c ? v[3] : w[3];
  v[3] = 40;
  w[3] = 80;
  long z = c ? v[0] : s;
  printf("%d %d %d | ", y, cr, int(z));
}

int main() {
  v4 v = {1, 2, 3, 4};
  printf("%d %d | ", v[0], 6);
  g(1);
  g(0);
  printf("end\n");
}
