// C17 6.7.9p6-p7, p17-p19: designators in any order, nested member and
// array designators (.a.b[3] =), array index designators, and a later
// initializer overriding an earlier one for the same subobject.
//@ expect: run
//@ args: -Wno-initializer-overrides
//@ stdout: 3 1 2 | 0 0 0 9 | 7 0 5 | 4 8
int printf(const char *, ...);
struct In { int b[4]; };
struct Out { int x, y, z; struct In a; };
struct Out o = { .z = 2, .x = 3, .y = 1, .a.b[3] = 9 };
int arr[3] = { [2] = 5, [0] = 7 };
int ov[2] = { [0] = 1, [1] = 8, [0] = 4 };
int main(void) {
  printf("%d %d %d | %d %d %d %d | %d %d %d | %d %d\n", o.x, o.y, o.z,
         o.a.b[0], o.a.b[1], o.a.b[2], o.a.b[3], arr[0], arr[1], arr[2],
         ov[0], ov[1]);
  return 0;
}
