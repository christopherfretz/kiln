// C23 6.7.11p2 (initialization): no initializer shall provide a value
// for an object not contained in the entity. clang only warns
// (-Wexcess-initializers, on by default; an error under
// -pedantic-errors) and drops the excess clauses: the object keeps its
// declared size, and an excess clause is never evaluated (no call, no
// side effect), at file scope and at block scope.
//@ expect: run
//@ args: -Wall
//@ diag: 22:19 warning "excess elements in array initializer"
//@ diag: 23:13 warning "excess elements in scalar initializer"
//@ diag: 25:21 warning "excess elements in struct initializer"
//@ diag: 26:13 warning "initializer-string for char array is too long"
//@ diag: 27:23 warning "excess elements in array initializer"
//@ diag: 28:20 warning "excess elements in char array initializer"
//@ diag: 35:22 warning "excess elements in array initializer"
//@ diag: 36:16 warning "excess elements in scalar initializer"
//@ diag: 37:24 warning "excess elements in struct initializer"
//@ diag: 38:21 warning "excess elements in scalar initializer"
//@ stdout: 8 1 2 | 4 3 | 8 5 6 | 3 abc | 4 1 2 3 4 | 3 xy | 8 7 8 | 9 | 1 2 5 | 0
int printf(const char *, ...);
static int calls;
int a[2] = {1, 2, 3};
int x = {3, 4};
struct S { int p, q; };
struct S s = {5, 6, 7};
char c[3] = "abcdef";
int n[2][2] = {{1, 2, 3}, {3, 4}};
char d[3] = {"xy", 'z'};
static int bump(void) { return ++calls; }
int main(void) {
  printf("%d %d %d | %d %d | %d %d %d | %d %.3s | %d %d %d %d %d | %d %s",
         (int)sizeof a, a[0], a[1], (int)sizeof x, x, (int)sizeof s, s.p, s.q,
         (int)sizeof c, c, (int)(sizeof n / sizeof n[0][0]), n[0][0], n[0][1], n[1][0],
         n[1][1], (int)sizeof d, d);
  int la[2] = {7, 8, bump()};
  int lx = {9, bump()};
  struct S ls = {1, 2, bump()};
  int lc = (int){5, bump()};
  printf(" | %d %d %d | %d | %d %d %d | %d\n", (int)sizeof la, la[0], la[1], lx, ls.p,
         ls.q, lc, calls);
  return 0;
}
