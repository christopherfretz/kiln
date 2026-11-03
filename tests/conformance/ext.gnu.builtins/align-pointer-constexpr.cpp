// [expr.const]: the alignment builtins on pointers (clang ExprConstant
// PointerExprEvaluator BI__builtin_align_up / _down, IntExprEvaluator
// BI__builtin_is_aligned) fold from the alignment known at compile time:
// the pointer's complete object's (a variable's declared alignment, else
// its type's) at the pointer's byte offset in it. `is_aligned` is true
// when that is at least the requested alignment, false when the object's
// is but the offset's is not; `align_up` / `align_down` are the pointer
// itself, or the pointer with its offset rounded when the object's
// alignment suffices. Anything else is non-constant, noted at the
// pointer. Same at clang 19 and 22.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 57:15 error "static assertion expression is not an integral constant expression"
//@ diag: 57:36 note "cannot constant evaluate whether run-time alignment is at least 32"
//@ diag: 58:15 error "static assertion expression is not an integral constant expression"
//@ diag: 58:34 note "cannot constant evaluate the result of adjusting alignment to 32"
//@ diag: 59:15 error "static assertion expression is not an integral constant expression"
//@ diag: 59:37 note "cannot constant evaluate whether run-time alignment is at least 8"
//@ diag: 60:15 error "static assertion expression is not an integral constant expression"
//@ diag: 60:36 note "cannot constant evaluate the result of adjusting alignment to 8"
//@ diag: 62:15 error "static assertion expression is not an integral constant expression"
//@ diag: 61:61 note "cannot constant evaluate whether run-time alignment is at least 64"
//@ diag: 62:15 note "in call to 'f(&arr[0])'"
alignas(16) char buf[64];
static_assert(__builtin_is_aligned(&buf[0], 16));
static_assert(__builtin_is_aligned(&buf[16], 16));
static_assert(!__builtin_is_aligned(&buf[1], 2));
static_assert(__builtin_is_aligned(&buf[4], 4) && !__builtin_is_aligned(&buf[4], 8));
static_assert(__builtin_align_up(&buf[1], 16) == &buf[16]);
static_assert(__builtin_align_up(&buf[16], 16) == &buf[16]);
static_assert(__builtin_align_down(&buf[17], 16) == &buf[16]);
static_assert(__builtin_align_up(&buf[63], 16) == buf + 64);
static_assert(__builtin_align_up((void *)&buf[1], 8) == (void *)&buf[8]);

int x;
static_assert(__builtin_is_aligned(&x, 4));
struct S { char c; int i; } s;
static_assert(__builtin_is_aligned(&s.i, 4));
static_assert(!__builtin_is_aligned(&s.c + 1, 2));
constexpr int arr[5] = {};
static_assert(__builtin_align_up(&arr[1], 4) == &arr[1]);

constexpr bool local() {
  alignas(8) char a[16]{};
  return __builtin_align_down(&a[15], 8) == a + 8 && __builtin_align_up(&a[3], 8) - a == 8;
}
static_assert(local());
constexpr bool heap() {
  int *p = new int[4];
  bool r = __builtin_is_aligned(p + 1, 4) && __builtin_align_up(p + 2, 4) == p + 2;
  delete[] p;
  return r;
}
static_assert(heap());

// The object's alignment is less than the requested one: unknown.
static_assert(__builtin_is_aligned(&buf[0], 32));
static_assert(__builtin_align_up(&buf[1], 32) == &buf[32]);
static_assert(!__builtin_is_aligned(&x, 8));
static_assert(__builtin_align_down(&arr[3], 8) == &arr[2]);
constexpr int f(const int *p) { return __builtin_is_aligned(p, 64); }
static_assert(f(arr) == 0);
