// __builtin_object_size / __builtin_dynamic_object_size in constant
// expressions (clang ExprConstant tryEvaluateBuiltinObjectSize): the bytes
// from the pointer to the end of the complete object (types 0 and 2) or of
// the closest enclosing array or member subobject (types 1 and 3); a past-
// the-end pointer gives 0; a heap allocation counts too. A null pointer or
// an object of unknown extent is not a constant ([expr.const]/10).
// Verified against clang 19.
//@ expect: compile-ok
struct S { char a[4]; int b; };
constexpr S s{};
constexpr char buf[10] = {};
static_assert(__builtin_object_size(buf, 0) == 10);
static_assert(__builtin_object_size(buf + 3, 0) == 7);
static_assert(__builtin_object_size(buf + 10, 0) == 0);
static_assert(__builtin_object_size(&s.a[1], 1) == 3);
static_assert(__builtin_object_size(&s.a[1], 0) == 7);
static_assert(__builtin_object_size(&s.a[1], 3) == 3);
static_assert(__builtin_object_size(&s.a[1], 2) == 7);
static_assert(__builtin_object_size(&s.b, 1) == 4);
static_assert(__builtin_dynamic_object_size(&s.b, 0) == 4);
static_assert(__builtin_dynamic_object_size(buf + 1, 3) == 9);
constexpr int f() {
  int *p = new int[5];
  auto r = __builtin_object_size(p + 1, 0);
  delete[] p;
  return r;
}
static_assert(f() == 16);
int g;
static_assert(__builtin_object_size(&g, 0) == 4);
template <class T, int K> constexpr unsigned long size_of(const T &t) {
  return __builtin_object_size(&t, K);
}
static_assert(size_of<S, 0>(s) == 8);
static_assert(size_of<S, 3>(s) == 8);
extern char ext[];
static_assert(!__builtin_constant_p(__builtin_object_size(ext, 0)));
