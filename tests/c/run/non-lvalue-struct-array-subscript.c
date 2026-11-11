// C17 6.5.2.1p2, 6.2.4p8: the array member of a structure returned by
// value decays, and subscripting it is an lvalue (an object of temporary
// lifetime), so it may be assigned to (gcc.c-torture
// compile/struct-non-lval-1.c, GCC bug 17855).
//@ std: c17
//@ expect: run
//@ stdout: 12 8
int printf(const char *, ...);
struct foo { char x, y, z[3]; int a[2]; };
__attribute__((noinline)) struct foo f(void) { struct foo r = { 1, 2, { 3, 4, 5 }, { 6, 7 } }; return r; }
int main(void) {
  int i = 1;
  f().z[i] = 9;
  int v = f().a[1] + f().z[2];
  printf("%d %d\n", v, (int)sizeof f().a);
  return 0;
}
