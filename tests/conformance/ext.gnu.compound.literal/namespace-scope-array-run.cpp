// Namespace-scope array compound literals ([GNU]; C99 6.5.2.5p5, clang's
// BuildCompoundLiteralExpr with a file-scope context): the literal is an
// lvalue naming an unnamed object of static storage duration, so the
// pointer it decays to stays valid and writes through it persist. A
// const-qualified literal is readable in constant expressions through
// its initializer ([expr.const]/4 as for a const variable).
//@ expect: run
//@ exit: 0
struct P { int a, b; };
int *p = (int[]){1, 2, 3};
const int *q = (const int[]){4, 5};
int (*r)[2] = &(int[2]){};
P *ps = (P[]){{1, 2}, {3, 4}};
const char *const *names = (const char *const[]){"a", "bc"};
namespace n { int *np = (int[]){9}; }
struct S { static inline int *sq = (int[]){7, 8}; };
template <class T> T *vt = (T[]){10, 20};
constexpr const int *kp = (const int[]){5, 6};
static_assert(kp[1] == 6);
constexpr int k0 = ((const int[]){3, 4})[0];
static_assert(k0 == 3);
auto sz = sizeof((int[]){1, 2, 3});
void bump() { p[1] += 40; (*r)[1] = 6; ps[1].b += 1; vt<int>[0] += 1; }
int read() { return p[1] + (*r)[1] + ps[1].b + vt<int>[0]; }
int main() {
  bump();
  if (read() != 42 + 6 + 5 + 11) return 1;
  if (q[0] + q[1] != 9 || names[1][1] != 'c' || n::np[0] != 9) return 2;
  if (S::sq[1] != 8 || (int)sz != 3 * (int)sizeof(int)) return 3;
  return kp[0] + k0 - 8;
}
