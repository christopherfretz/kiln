// [GNU] compound literals (C99 6.5.2.5p5): outside a function an array
// compound literal is an lvalue naming an object of static storage
// duration, an internal `.compoundliteral` global (`internal constant`
// for a const element type) defined at its first use after the
// constants of its initializer, numbered by the module's name uniquer.
// Clang 19 and 22 emit the same globals (WP-466).
struct P { int a, b; };
int *p = (int[]){1, 2, 3};
const int *q = (const int[]){4, 5};
int *r = (int[2]){};
P *ps = (P[]){{1, 2}, {3, 4}};
const char *const *names = (const char *const[]){"a", "bc"};
namespace n { int *np = (int[]){9}; }
extern "C" { int *cp = (int[]){10, 11}; }
unsigned long sz = sizeof((int[]){1, 2, 3});
constexpr const int *kp = (const int[]){5, 6};
int k1 = kp[1];
int x = ((int[]){7, 8})[1];
int *mid = (int[]){1, 2, 3} + 1;
int &ref = *(int[]){42};
static int *sp = (int[]){7};
int **pp = (int *[]){nullptr, (int[]){3}};
struct S { static inline int *sq = (int[]){3}; };
struct D { int *dp = (int[]){1, 2}; };
D d;
template <class T> T *vt = (T[]){1, 2};
int *use() { return sp; }
int *usev() { return vt<int>; }
int *uses() { return S::sq; }
