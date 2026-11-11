// C17 6.8.2 (mixed declarations and statements), 6.8.5p5 (for
// declarations), 6.4.2.2 (__func__), 6.7.10 (_Static_assert), 6.7.4
// (_Noreturn), 6.7.5 (_Alignas), 6.5.3.4 (_Alignof), 6.7.1
// (_Thread_local), 6.7.2.1p13 (anonymous structs and unions), 6.5.1.1
// (_Generic applies lvalue conversion to its controlling expression).
//@ expect: run
//@ stdout: main 16 8 3 7 1 1
int printf(const char *, ...);
_Noreturn void die(void);
_Thread_local int tl = 3;
struct A { _Alignas(16) char c; };
struct U { int k; union { int i; float f; }; struct { int lo, hi; }; };
int main(void) {
  printf("%s ", __func__);
  int x = 1;
  x++;
  int y = x * 2;
  for (int i = 0; i < 2; i++) y++;
  _Static_assert(_Alignof(struct A) == 16, "align");
  struct U u = { .k = 0 };
  u.i = 7;
  u.hi = 1;
  const int ci = 5;
  printf("%zu %zu %d %d %d %d\n", sizeof(struct A), _Alignof(double), tl, u.i,
         _Generic(ci, int: 1, default: 0), _Generic("s", char *: 1, default: 0));
  return y - 6;
}
