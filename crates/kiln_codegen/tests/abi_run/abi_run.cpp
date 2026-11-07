// The run-time half of the calling-convention matrix (spec/landed/abi/
// WP-766.md; tests/abi_run_vs_clang.rs): this file is compiled twice, once with
// `-DABI_CALLER` and once without, by two different compilers (kiln and
// clang, then the other way round), and the two objects are linked. The
// caller passes each shape to the callee by value — as a named argument,
// after the argument registers are used up, and through `...` — and gets
// one back; both sides check every member ([expr.call]/7, Itanium C++ ABI
// §3.1.2, AAPCS64 §6.8, SysV x86-64 psABI §3.2.3). Exit status 0 and
// `ok` on stdout mean the two compilers agree.
// No system headers: the file also goes through cross-target IR checks.
extern "C" int printf(const char *, ...);

#pragma clang diagnostic ignored "-Wreturn-type-c-linkage"

#ifdef ABI_CALLER
int abi_failures;
#else
extern int abi_failures;
#endif

template <class T> T make(int &s);

template <class M> void put(M &m, int &s) { m = make<M>(s); }
template <class M, unsigned N> void put(M (&m)[N], int &s) {
  for (unsigned i = 0; i < N; ++i)
    put(m[i], s);
}

template <class T> T make(int &s) {
  if constexpr (__is_union(T)) {
    T t;
    unsigned char b[sizeof(T)];
    for (unsigned i = 0; i < sizeof(T); ++i)
      b[i] = (unsigned char)(s++ * 37 + 11);
    __builtin_memcpy(&t, b, sizeof(T));
    return t;
  } else if constexpr (__is_class(T)) {
    T t;
    auto &[... x] = t;
    if constexpr (requires { T::bits; })
      ((x = make<__remove_cvref(decltype(x))>(s)), ...);
    else
      (put(x, s), ...);
    return t;
  } else if constexpr (__is_pointer(T)) {
    return (T)(__UINTPTR_TYPE__)(0x1000 + 16 * s++);
  } else {
    return (T)(s++ % 23 + 1);
  }
}

template <class T> bool same(const T &a, const T &b) {
  if constexpr (__is_union(T)) {
    return __builtin_memcmp(&a, &b, sizeof(T)) == 0;
  } else if constexpr (__is_class(T)) {
    auto &[... x] = a;
    auto &[... y] = b;
    return (same(x, y) && ...);
  } else if constexpr (__is_array(T)) {
    for (unsigned i = 0; i < sizeof(T) / sizeof(a[0]); ++i)
      if (!same(a[i], b[i]))
        return false;
    return true;
  } else {
    return a == b;
  }
}

template <class T> T expect(int seed) {
  int s = seed;
  return make<T>(s);
}

template <class T> void check(const T &v, int seed, const char *shape, const char *where) {
  if (!same(v, expect<T>(seed))) {
    ++abi_failures;
    printf("%s: %s differs\n", shape, where);
  }
}

#define GPRS long, long, long, long, long, long, long, long
#define FPRS double, double, double, double, double, double, double, double
#define GPR_ARGS 1, 2, 3, 4, 5, 6, 7, 8
#define FPR_ARGS 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0

// The functions one shape exchanges; the callee defines them, the caller
// calls them.
#define DECLS(T)                                                              \
  T T##_echo(T a, int seed);                                                  \
  T T##_stack(GPRS, FPRS, T a, int seed);                                     \
  extern "C" T T##_c(int seed, T a);

#ifdef ABI_CALLER
#define SHAPE(T)                                                              \
  DECLS(T)                                                                    \
  T T##_vecho(int seed, ...);                                                 \
  static void T##_run(int seed) {                                             \
    check(T##_echo(expect<T>(seed), seed), seed + 1, #T, "echo result");      \
    check(T##_stack(GPR_ARGS, FPR_ARGS, expect<T>(seed + 2), seed + 2),       \
          seed + 3, #T, "stack result");                                      \
    check(T##_c(seed + 4, expect<T>(seed + 4)), seed + 5, #T, "C result");    \
    check(T##_vecho(seed + 6, expect<T>(seed + 6)), seed + 7, #T,             \
          "variadic result");                                                 \
  }
#define SHAPE_NV(T)                                                           \
  DECLS(T)                                                                    \
  static void T##_run(int seed) {                                             \
    check(T##_echo(expect<T>(seed), seed), seed + 1, #T, "echo result");      \
    check(T##_stack(GPR_ARGS, FPR_ARGS, expect<T>(seed + 2), seed + 2),       \
          seed + 3, #T, "stack result");                                      \
    check(T##_c(seed + 4, expect<T>(seed + 4)), seed + 5, #T, "C result");    \
  }
#else
#define SHAPE_NV(T)                                                           \
  DECLS(T)                                                                    \
  T T##_echo(T a, int seed) {                                                 \
    check(a, seed, #T, "echo argument");                                      \
    return expect<T>(seed + 1);                                               \
  }                                                                           \
  T T##_stack(GPRS, FPRS, T a, int seed) {                                    \
    check(a, seed, #T, "stack argument");                                     \
    return expect<T>(seed + 1);                                               \
  }                                                                           \
  extern "C" T T##_c(int seed, T a) {                                         \
    check(a, seed, #T, "C argument");                                         \
    return expect<T>(seed + 1);                                               \
  }
#define SHAPE(T)                                                              \
  SHAPE_NV(T)                                                                 \
  T T##_vecho(int seed, ...) {                                                \
    __builtin_va_list ap;                                                     \
    __builtin_va_start(ap, seed);                                             \
    T a = __builtin_va_arg(ap, T);                                            \
    __builtin_va_end(ap);                                                     \
    check(a, seed, #T, "variadic argument");                                  \
    return expect<T>(seed + 1);                                               \
  }
#endif

struct E {};
struct FENua { float f; [[no_unique_address]] E e; };
struct HfaNua5 { float a, b, c, d; [[no_unique_address]] E e; };
struct LE { long l; E e; };
struct O8 { [[no_unique_address]] E pad[8]; long b; };
struct P1 { char a[1]; };
struct P3 { char a[3]; };
struct P9 { char a[9]; };
struct P12 { char a[12]; };
struct P17 { char a[17]; };
struct P33 { char a[33]; };
struct S3 { short a, b, c; };
struct I3 { int a, b, c; };
struct L3 { long a, b, c; };
struct Ptr2 { void *a, *b; };
struct F3 { float a, b, c; };
struct F4 { float a[4]; };
struct D4 { double a, b, c, d; };
struct D5 { double a[5]; };
struct H3 { _Float16 a, b, c; };
struct FD { float f; double d; };
struct IF { int i; float f; };
struct DI { double d; int i; };
struct FFI { float a, b; int c; };
struct CD { char c; double d; };
struct alignas(16) A16L { long l; };
struct alignas(32) A32C { char c; };
struct B { static constexpr bool bits = true; int a : 3; unsigned b : 7; int : 0; short c : 9; };
union UIF { int i; float f; };
union UDL { double d; long l; };
struct LDv { long double v; };
struct I128 { __int128 v; };
struct SLDI { long double d; int i; };
struct NCopy {
  int a, b;
  NCopy() = default;
  NCopy(const NCopy &o) : a(o.a), b(o.b) {}
};

#define SHAPES(X)                                                             \
  X(E) X(FENua) X(HfaNua5) X(LE) X(P1) X(P3) X(P9) X(P12) X(P17) X(P33)       \
  X(S3) X(I3) X(L3) X(Ptr2) X(F3) X(F4) X(D4) X(D5) X(H3) X(FD) X(IF) X(DI)   \
  X(FFI) X(CD) X(A16L) X(A32C) X(B) X(UIF) X(UDL) X(LDv) X(I128) X(SLDI)
#define SHAPES_NV(X) X(O8) X(NCopy)

SHAPES(SHAPE)
SHAPES_NV(SHAPE_NV)

#ifdef ABI_CALLER
int main() {
  int seed = 1;
#define RUN(T) T##_run(seed += 10);
  SHAPES(RUN)
  SHAPES_NV(RUN)
  if (abi_failures == 0)
    printf("ok\n");
  return abi_failures != 0;
}
#endif
