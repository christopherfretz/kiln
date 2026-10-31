// [expr.prim.id.qual], [basic.lookup.qual.general]: a builtin function
// is declared in the global namespace, so `::__builtin_x` names it as
// the unqualified name does (libc++'s constexpr_c_functions.h spells
// `::__builtin_memmove`). Not a builtin: still "no member named".
//@ expect: compile-ok
//@ strict-diags
typedef decltype(sizeof 0) size_t;

static_assert(::__builtin_strlen("hello") == 5);
static_assert(::__builtin_memcmp("abc", "abd", 3) < 0);

template <class T>
constexpr T *copy(T *dst, const T *src, size_t n) {
  ::__builtin_memmove(dst, src, n * sizeof(T));
  return dst;
}

constexpr bool f() {
  int a[3] = {1, 2, 3};
  int b[3] = {};
  copy(b, a, 3);
  return b[0] == 1 && b[2] == 3 && ::__builtin_is_constant_evaluated();
}
static_assert(f());

namespace n {
  inline size_t len(const char *s) { return ::__builtin_strlen(s); }
}

int main() { return n::len("ab") == 2 ? 0 : 1; }
