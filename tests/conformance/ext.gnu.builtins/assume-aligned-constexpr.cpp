// __builtin_assume_aligned in a constant expression (clang 19
// PointerExprEvaluator, BI__builtin_assume_aligned): the complete object
// must be aligned to the asserted alignment (a variable's declared
// alignment, `getDeclAlign`, else its type's), and the pointer's byte
// offset in it minus the optional offset must be a multiple of it; a
// pointer to no object is checked by value.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 29:15 error "static assertion expression is not an integral constant expression"
//@ diag: 29:40 note "alignment of the base pointee object (4 bytes) is less than the asserted 8 bytes"
//@ diag: 30:15 error "static assertion expression is not an integral constant expression"
//@ diag: 30:44 note "offset of the aligned pointer from the base pointee object (2 bytes) is not a multiple of the asserted 4 bytes"
//@ diag: 32:15 error "static assertion expression is not an integral constant expression"
//@ diag: 32:40 note "offset of the aligned pointer from the base pointee object (5 bytes) is not a multiple of the asserted 2 bytes"
//@ diag: 35:15 error "static assertion expression is not an integral constant expression"
//@ diag: 35:40 note "value of the aligned pointer (-1) is not a multiple of the asserted 4 bytes"
//@ diag: 36:15 error "static assertion expression is not an integral constant expression"
//@ diag: 36:40 note "alignment of the base pointee object (1 byte) is less than the asserted 2 bytes"
//@ diag: 40:15 error "static assertion expression is not an integral constant expression"
//@ diag: 39:78 note "alignment of the base pointee object (4 bytes) is less than the asserted 8 bytes"
//@ diag: 40:15 note "in call to 'f3()'"
struct S { int a; char c[8]; };
constexpr int x = 1;
alignas(16) constexpr char buf[4] = {};
constexpr S s = {};
constexpr bool f1() { int v = 0; return __builtin_assume_aligned(&v, 4) == &v; }
static_assert(f1());
static_assert(__builtin_assume_aligned(buf, 16) == buf);
static_assert(__builtin_assume_aligned(&x, 8) == &x);
static_assert(__builtin_assume_aligned(buf + 2, 4) == buf);
static_assert(__builtin_assume_aligned(buf + 2, 4, 2) == buf + 2);
static_assert(__builtin_assume_aligned(&s.c[1], 2) != nullptr);
static_assert(__builtin_assume_aligned(&s.c[1], 2, 5) != nullptr);
static_assert(__builtin_assume_aligned(nullptr, 4) == nullptr);
static_assert(__builtin_assume_aligned(nullptr, 4, 1) == nullptr);
static_assert(__builtin_assume_aligned("abc", 2) != nullptr);
constexpr bool f2() { int *p = new int[2]; bool r = __builtin_assume_aligned(p + 1, 4) == p + 1; delete[] p; return r; }
static_assert(f2());
constexpr bool f3() { int *p = new int[2]; bool r = __builtin_assume_aligned(p, 8) == p; delete[] p; return r; }
static_assert(f3());
