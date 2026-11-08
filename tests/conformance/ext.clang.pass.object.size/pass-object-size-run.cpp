// clang's `pass_object_size(N)` / `overloadable` (WP-803; glibc's
// `_FORTIFY_SOURCE` headers declare the printf and string functions
// so): a hidden `size_t` argument follows each such parameter, the
// object size of the argument, which `__builtin_object_size(p, t)` in the
// callee reads for a compatible `t`; a pass-through argument passes the
// caller's own. Such a function overloads, and outranks, the one without,
// and is mangled with `U17pass_object_size<N>` even with C language
// linkage. Verified against clang 19.
//@ expect: run
//@ stdout: 32 32 32|16 16|5 0 7|9|1 2|8 6 0
extern "C" int printf(const char *, ...);
typedef __SIZE_TYPE__ size_t;

static size_t s0(const char *const __attribute__((pass_object_size(0))) p) {
  return __builtin_object_size(p, 0);
}
static size_t s1(const char *const __attribute__((pass_object_size(1))) p) {
  return __builtin_object_size(p, 1);
}
// Type 3 of a pointer whose object size is not a constant is 0.
static size_t s3(const char *const __attribute__((pass_object_size(3))) p) {
  return __builtin_object_size(p, 3);
}
// Pass-through: the caller's hidden argument, not the callee's view.
static size_t outer(char *const __attribute__((pass_object_size(0))) p) { return s0(p); }

extern "C" {
int pick(const char *p) { return 1; }
static int __attribute__((overloadable))
pick(const char *const __attribute__((pass_object_size(0))) p) { return 2; }
}

// External linkage: called by its mangled name below (the hidden size is
// the second argument).
size_t named(const char *const __attribute__((pass_object_size(0))) p, int k) {
  return __builtin_object_size(p, 0) + k;
}
extern "C" size_t named_raw(const char *, size_t, int) __asm__("_Z5namedPKcU17pass_object_size0i");

int main() {
  char buf[32];
  printf("%zu %zu %zu|", s0(buf), s1(buf), s3(buf));
  char small[16];
  printf("%zu %zu|", outer(small), s0(small + 0));
  printf("%zu %zu %zu|", named_raw(buf, 5, 0), named_raw(buf, 0, 0), named(buf + 30, 5));
  printf("%zu|", named(small + 4, -3));
  int (*fp)(const char *) = pick;
  struct { char a[8]; int b; } st;
  char *q = buf;
  printf("%d %d|", fp(buf), pick(buf));
  printf("%zu %zu %zu\n", s1(st.a), s3(st.a + 2), s3(q));
}
