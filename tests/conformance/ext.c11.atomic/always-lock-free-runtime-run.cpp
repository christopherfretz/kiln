// [driver] `__atomic_always_lock_free` with a size that is not a constant
// expression (GNU; clang's codegen says "cannot compile this builtin
// function yet", GCC accepts it once optimization makes the size
// constant): kiln computes the constant fold's rule at run time, so
// the answer is the one the same call with a constant size gets. A
// power-of-two size up to the target's inline atomic width is lock-free
// for a null object, a size of 1, or an object at least that aligned (an
// integer address must be a multiple of the size). Sizes up to 8 only,
// which both targets answer alike. Found running GCC's C++ testsuite.
// WP-1739.
//@ expect: run
//@ skip(clang): clang's codegen rejects the non-constant size
//@ stdout: 1 1 1 0 0 1 1 0
//@ stdout: 1 1 1 0 0 1 1 0
extern "C" int printf(const char *, ...);
struct S { char c[3]; };
int *s;
__attribute__((noinline)) unsigned long sz(unsigned long n) { return n; }
int main() {
  char c;
  long l;
  S *ps = nullptr;
  printf("%d %d %d %d %d %d %d %d\n", __atomic_always_lock_free(sz(4), s),
         __atomic_always_lock_free(sz(8), 0), __atomic_always_lock_free(sz(1), &c),
         __atomic_always_lock_free(sz(3), 0), __atomic_always_lock_free(sz(4), ps),
         __atomic_always_lock_free(sz(8), &l), __atomic_always_lock_free(sz(4), (void *)8),
         __atomic_always_lock_free(sz(2), (void *)3));
  printf("%d %d %d %d %d %d %d %d\n", __atomic_always_lock_free(4, s),
         __atomic_always_lock_free(8, 0), __atomic_always_lock_free(1, &c),
         __atomic_always_lock_free(3, 0), __atomic_always_lock_free(4, ps),
         __atomic_always_lock_free(8, &l), __atomic_always_lock_free(4, (void *)8),
         __atomic_always_lock_free(2, (void *)3));
}
