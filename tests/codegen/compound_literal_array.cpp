// A C99 array compound literal in C++ is a prvalue of the written array
// type (its unknown bound deduced from the list), materialized when bound
// to a reference or decayed to a pointer ([conv.array]/1).
constexpr int sum2(int (&&a)[2]) { return a[0] + a[1]; }
constexpr int sum3(int (&&a)[3]) { return a[0] + a[1] + a[2]; }
int f(int k) { return sum2((int[]){3, k}); }
int g(int k) { return sum3((int[3]){k}); }
int h(const int* p);
int i(int k) { return h((int[]){k, 2, 3}); }
constexpr int c() { return sum3((int[]){1, 2, 3}); }
static_assert(c() == 6);
int c2() { return c(); }
struct P { int a; double b; };
double j(int k) { auto&& r = (P[]){{k, 1.5}, {2}}; return r[0].b + r[1].a; }
long big(int k) { const long (&r)[8] = (const long[8]){k, 0, 0, 0, 0, 0, 0, 1}; return r[7]; }
