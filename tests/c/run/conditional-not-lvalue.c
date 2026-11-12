// C17 6.5.15p4 and footnote 110: a conditional expression does not
// yield an lvalue, so `c ? s1 : s2` of structure type is a value: its
// members are read from a copy of the chosen operand, and a `void`
// conditional only runs the chosen side.
//@ expect: run
//@ stdout: 2 7 | 1 9 | 3 4 | g
int printf(const char *, ...);
struct S { int a, b; } s1 = {1, 9}, s2 = {2, 7};
static void f(void) { printf("f"); }
static void g(void) { printf("g"); }
static struct S pick(int c) { return c ? s1 : s2; }
int main(void) {
  struct S r = pick(0);
  int c = 1;
  printf("%d %d | %d %d | ", r.a, r.b, (c ? s1 : s2).a, (c ? s1 : s2).b);
  struct S t = c ? (struct S){3, 4} : s2;
  printf("%d %d | ", t.a, t.b);
  c = 0;
  c ? f() : g();
  printf("\n");
  return 0;
}
