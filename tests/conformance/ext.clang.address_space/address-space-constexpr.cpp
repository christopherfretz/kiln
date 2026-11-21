// Constant evaluation looks through address spaces (WP-1803): a pointer or
// reference converted between spaces designates the same object, so the
// conversions are no-ops in the evaluator, at namespace scope and in a
// constexpr function's locals.
//@ expect: compile-ok
//@ strict-diags
#define AS1 __attribute__((address_space(1)))
#define GLOBAL __attribute__((opencl_global))
#define GENERIC __attribute__((opencl_generic))

AS1 int g = 3;
constexpr AS1 int *p = &g;
constexpr AS1 int *n = nullptr;
static_assert(p != nullptr && n == nullptr);
static_assert(p == &g);
constexpr int *q = (int *)&g;
static_assert((AS1 int *)q == p);
constexpr AS1 const int cg = 5;
static_assert(cg == 5);
constexpr int rd() { return *(AS1 const int *)&cg; }
static_assert(rd() == 5);
constexpr AS1 int *pp = (AS1 int *)(int *)nullptr;
static_assert(pp == nullptr);
constexpr AS1 const int arr[3] = {1, 2, 3};
static_assert(arr[2] == 3 && *(arr + 1) == 2);
constexpr bool local() {
  int x = 1;
  AS1 int *a = (AS1 int *)&x;
  *a = 7;
  int *b = (int *)a;
  return x == 7 && b == &x;
}
static_assert(local());
GLOBAL const int og = 11;
constexpr int generic() {
  GENERIC const int *g = &og;
  GENERIC const int &r = og;
  return *g + r;
}
static_assert(generic() == 22);
struct S { int a, b; };
constexpr AS1 S gs = {4, 5};
static_assert(gs.b == 5);
constexpr const AS1 int &mb = gs.b;
static_assert(mb == 5);
constexpr int sum(const AS1 int *p, int k) {
  int s = 0;
  for (int i = 0; i < k; ++i) s += p[i];
  return s;
}
static_assert(sum(arr, 3) == 6);
static_assert(sum(&gs.b, 1) == 5);
