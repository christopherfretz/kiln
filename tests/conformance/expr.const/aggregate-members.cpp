// [expr.const]/5: reads of constexpr aggregate members and array elements
//@ expect: compile-fail
//@ strict-diags
//@ diag: 14:15 error "static assertion expression is not an integral constant expression"
//@ diag: 14:15 note "read of dereferenced one-past-the-end pointer is not allowed in a constant expression"
//@ diag: 15:15 error "static assertion expression is not an integral constant expression"
//@ diag: 15:15 note "cannot refer to element -1 of array of 3 elements in a constant expression"
struct P { int x; long y; double d; bool b; char c[3]; };
constexpr P p = {3, 4, 2.5, true, {'a', 'b'}};
constexpr int a[3] = {1, 2, 3};
constexpr P ps[2] = {{1}, {2, 5}};
static_assert(p.x + p.y == 7 && a[2] == 3 && 2[a] == 3 && p.d == 2.5 && p.b && p.c[1] == 'b' && p.c[2] == 0);
static_assert(ps[1].y == 5 && ps[0].y == 0 && (ps[1]).x == 2);
static_assert(a[3] == 0);
static_assert(a[-1] == 0);
constexpr int k = a[1] * p.x;
static_assert(k == 6);
int arr[a[2]];
