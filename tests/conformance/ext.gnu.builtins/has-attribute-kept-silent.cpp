// GCC's __builtin_has_attribute answers 1 for an attribute kiln accepts
// silently wherever GCC keeps it: `nonstring` on a variable, parameter or
// member whose type is an array of (or pointer to) a narrow character type,
// `nocommon` on a variable that is not a parameter; elsewhere both drop it
// and answer 0. `nonnull` on a typedef, alias, variable or member of
// prototyped function (pointer or reference) type is kept, as clang 22 does
// (no warning), and asked through the typedef; on a pointer to function GCC
// keeps it on the pointee's type, so the pointer answers 0. `format` may give only the
// leading arguments; those given must match. Found running GCC's C++
// testsuite (c-c++-common/builtin-has-attribute-2/-3/-4). WP-1675.
//@ std: c++20
//@ expect: compile-ok
//@ requires: clang>=22
//@ oracle: gcc
//@ skip(clang): clang 19/22 have no __builtin_has_attribute
//@ strict-diags
#define HAS(x, ...) __builtin_has_attribute(x, __VA_ARGS__)

__attribute__((nonstring)) char ns[4];
__attribute__((nonstring)) const char *nsp;
__attribute__((nonstring)) unsigned char ns2[2][3];
typedef char C4[4];
[[gnu::nonstring]] C4 ns4;
char plain[4];
struct M { __attribute__((nonstring)) char f[2]; char g[2]; } m;
void par(__attribute__((nonstring)) char *p, char *q) {
  static_assert(HAS(p, nonstring) && !HAS(q, nonstring));
}
static_assert(HAS(ns, nonstring) && HAS(nsp, nonstring) && HAS(ns2, __nonstring__));
static_assert(HAS(ns4, nonstring) && !HAS(plain, nonstring));
static_assert(HAS(m.f, nonstring) && !HAS(m.g, nonstring));
static_assert(!HAS(C4, nonstring));

__attribute__((nocommon)) int nc;
__attribute__((nocommon)) static int snc;
[[gnu::nocommon]] extern int enc;
struct SM { __attribute__((nocommon)) static int s; };
int pc;
static_assert(HAS(nc, nocommon) && HAS(snc, nocommon) && HAS(enc, nocommon));
static_assert(HAS(SM::s, nocommon) && !HAS(pc, nocommon));
void loc() {
  __attribute__((nocommon)) int l;
  static_assert(HAS(l, nocommon));
}

typedef void F3(void *, void *, void *);
typedef __attribute__((nonnull)) F3 Fall;
typedef __attribute__((nonnull(2))) void F2(void *, void *, void *);
typedef __attribute__((nonnull(1), nonnull(3))) void F13(void *, void *, void *);
using A1 __attribute__((nonnull(1))) = void(void *);
typedef __attribute__((nonnull(2))) void (*FP2)(int, void *);
__attribute__((nonnull)) void (*vp)(void *);
F2 fd;
static_assert(!HAS(F3, nonnull));
static_assert(HAS(Fall, nonnull) && HAS(Fall, nonnull(1)) && HAS(Fall, nonnull(3)));
static_assert(!HAS(F2, nonnull) && HAS(F2, nonnull(2)) && !HAS(F2, nonnull(1)));
static_assert(HAS(F13, nonnull(1)) && HAS(F13, nonnull(3)) && !HAS(F13, nonnull(2)));
static_assert(HAS(A1, nonnull(1)));
// GCC puts it on the function type a pointer points to: not the pointer's.
static_assert(!HAS(FP2, nonnull(2)) && !HAS(vp, nonnull));
static_assert(HAS(fd, nonnull(2)) && !HAS(fd, nonnull(1)));

__attribute__((format(printf, 2, 3))) void ff(int, const char *, ...);
__attribute__((format(scanf, 1, 2))) int fs(const char *, ...);
void fnone(int, const char *, ...);
static_assert(HAS(ff, format) && HAS(ff, format(printf)) && HAS(ff, format(printf, 2)));
static_assert(HAS(ff, format(printf, 2, 3)) && !HAS(ff, format(scanf)));
static_assert(HAS(fs, format(scanf, 1)) && !HAS(fs, format(printf)));
static_assert(!HAS(fnone, format) && !HAS(fnone, format(printf)) && !HAS(fnone, format(printf, 2)));

int main() { return nc + snc + pc + plain[0] + ns[0] + m.f[0]; }
int enc, SM::s;
