// [basic.def.odr]/5.2, [dcl.init.aggr], spec/sema.md §1.2: a braced
// initializer of 65,536 elements, each naming a constant variable through
// a subscript, a member access or directly. Each name is a potential
// result of an lvalue-to-rvalue conversion, so it is not an odr-use; the
// full-expression's pending references are tested against those
// candidates one by one, and that test must not scan them all: before
// WP-1335 this compile was quadratic (11 s at 262,144 elements in a
// release build, 0.9 s after; the bench entry
// bench/frontend/constant-table-emit is the timed shape).
//@ expect: compile-ok
//@ strict-diags
//@ mem<=512
struct P { int m; };
static constexpr int a[2] = {1, 2};
static constexpr P s = {3};
static constexpr int v = 4;
#define M0 a[0], a[1], s.m, v,
#define M2  M0  M0  M0  M0
#define M4  M2  M2  M2  M2
#define M6  M4  M4  M4  M4
#define M8  M6  M6  M6  M6
#define M10 M8  M8  M8  M8
#define M12 M10 M10 M10 M10
#define M14 M12 M12 M12 M12

constexpr int t[] = {M14};
static_assert(sizeof(t) / sizeof(t[0]) == 65536);
static_assert(t[65533] == 2 && t[65534] == 3 && t[65535] == 4);
int main() { return t[0] - 1; }
