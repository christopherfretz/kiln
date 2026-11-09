// GNU __builtin_object_size / __builtin_dynamic_object_size of a pointer
// into the result of a call to an `alloc_size` function (clang
// tryEvaluateBuiltinObjectSize with an invalid base,
// getBytesReturnedByAllocSizeCall): the library malloc / calloc / realloc /
// aligned_alloc get an implicit `alloc_size` (Sema::AddKnownFunctionAttributes),
// and a user one counts too. The size is the product of the constant size
// arguments (an overflowing product is unknown), less the byte offset of
// the pointer, 0 before the start or past the end, for every type. A local
// `const` pointer initialized by such a call designates it as well. A cast
// to a non-void pointer inside the operand invalidates the designator, so
// type 3 (a lower bound) is then unknown: 0. A size argument that is not
// constant is unknown: the side-effect default. With an invalid designator
// and no side effects, type 3 is not a constant at all. WP-1073.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 54:17 error "static assertion expression is not an integral constant expression"
typedef __SIZE_TYPE__ size_t;
extern "C" {
void *malloc(size_t);
void *calloc(size_t, size_t);
void *realloc(void *, size_t);
void *aligned_alloc(size_t, size_t);
}
void *my_alloc(int, size_t) __attribute__((alloc_size(2)));
void *my_alloc2(size_t, size_t) __attribute__((alloc_size(1, 2)));
constexpr size_t N = 24;
static_assert(__builtin_object_size(malloc(10), 0) == 10);
static_assert(__builtin_object_size(malloc(10), 1) == 10);
static_assert(__builtin_object_size(malloc(10), 2) == 10);
static_assert(__builtin_object_size(malloc(10), 3) == 10);
static_assert(__builtin_object_size(calloc(3, sizeof(int)), 0) == 12);
static_assert(__builtin_object_size(realloc(nullptr, 7), 1) == 7);
static_assert(__builtin_object_size(aligned_alloc(16, 32), 0) == 32);
static_assert(__builtin_dynamic_object_size(malloc(10), 1) == 10);
static_assert(__builtin_object_size(malloc(N), 0) == 24);
static_assert(__builtin_object_size(malloc(sizeof(double) * 2 + 1), 0) == 17);
static_assert(__builtin_object_size((char *)malloc(10), 3) == 10);
static_assert(__builtin_object_size((char *)malloc(10) + 3, 0) == 7);
static_assert(__builtin_object_size((char *)malloc(10) + 3, 1) == 7);
static_assert(__builtin_object_size((char *)malloc(10) + 3, 2) == 7);
static_assert(__builtin_object_size((char *)malloc(10) + 3, 3) == 0);
static_assert(__builtin_object_size(1 + (char *)malloc(10), 0) == 9);
static_assert(__builtin_object_size((int *)malloc(10) + 1, 0) == 6);
static_assert(__builtin_object_size((int *)malloc(10) + 3, 0) == 0);
static_assert(__builtin_object_size((char *)malloc(10) - 1, 0) == 0);
static_assert(__builtin_object_size(my_alloc(1, 5), 0) == 5);
static_assert(__builtin_object_size(my_alloc2(5, 6), 2) == 30);
static_assert(__builtin_object_size(my_alloc2(~(size_t)0, 2), 0) == (size_t)-1);
static_assert(__builtin_object_size(my_alloc2(~(size_t)0, 2), 2) == 0);
void f(size_t n) {
  char *const p = (char *)malloc(10);
  static_assert(__builtin_object_size(p, 0) == 10);
  static_assert(__builtin_object_size(p + 2, 3) == 8);
  static_assert(__builtin_object_size((int *)p + 1, 3) == 0);
  static_assert(__builtin_object_size(malloc(n), 0) == (size_t)-1);
  static_assert(__builtin_object_size(malloc(n), 2) == 0);
}
