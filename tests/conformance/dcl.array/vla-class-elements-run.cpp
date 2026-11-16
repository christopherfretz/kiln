//@ expect: run
//@ strict-diags
//@ args: -std=gnu++20
//@ stdout: 31 9 12
//@ stdout: caught 4 4
// A variable length array (GNU extension) of a class type with a
// non-trivial default constructor or destructor: the elements are
// default-initialized in order ([dcl.init.general]/7.2) and destroyed in
// reverse order when the array's scope is left ([class.dtor]/14), also by
// `break` and by an exception, and an empty array constructs nothing.
// Found running GCC's C++ testsuite.
extern "C" int printf(const char *, ...);

int ctor, dtor;
struct S {
  int v;
  S() : v(7) { ++ctor; }
  ~S() { ++dtor; }
};
struct T {
  int v;
  ~T() { ++dtor; }
};
struct U {
  int v = 3;
};
int f(int n) {
  int sum = 0;
  { S a[n]; for (int i = 0; i < n; ++i) sum += a[i].v; }
  { T b[n]; (void)b; }
  { U c[n][n]; sum += c[n - 1][n - 1].v; }
  for (int k = 0; k < 2; ++k) { S d[n]; if (k == 1) break; sum += d[0].v; }
  { S e[0 * n]; (void)e; }
  return sum;
}

int thrown;
struct X {
  X() { if (thrown == 4) throw 1; ++thrown; }
  ~X() { ++dtor; }
};
void g(int n) { X a[n]; X b[n]; }

int main() {
  int s = f(3);
  printf("%d %d %d\n", s, ctor, dtor);
  dtor = 0;
  try { g(3); } catch (int) { printf("caught %d %d\n", thrown, dtor); }
}
