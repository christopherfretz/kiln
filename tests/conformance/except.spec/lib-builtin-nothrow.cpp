// [except.spec]/12, [expr.unary.noexcept]: clang recognizes the first
// declaration of a C library function with C language linkage at
// namespace scope whose type matches its library-builtin signature
// (ignoring the exception specification) and gives it an implicit
// NoThrowAttr, so a call is non-throwing; redeclarations inherit it.
// `printf` and `malloc` are library builtins without NoThrow; a
// C++-linkage, block-scope or mismatched declaration is not recognized,
// and neither is a `FILE` function declared before the `FILE` typedef.
//@ expect: compile-ok
//@ args: --target=aarch64-unknown-linux-gnu
//@ strict-diags
extern "C" int vfprintf(struct _IO_FILE *, const char *, __builtin_va_list);
typedef struct _IO_FILE FILE;
extern "C" {
int fprintf(FILE *, const char *, ...);
int vsnprintf(char *, unsigned long, const char *, __builtin_va_list);
unsigned long strlen(const char *);
double sin(double);
int isdigit(int);
int printf(const char *, ...);
void *malloc(unsigned long);
}
namespace n { extern "C" double cos(double); }
extern "C" unsigned long strlen(const char *);
extern "C++" double tan(double);
extern "C" int strncmp(const char *, const char *, int);
struct S { static int abs(int); };

static_assert(noexcept(strlen("a")));
static_assert(noexcept(fprintf(nullptr, "x")));
static_assert(noexcept(n::cos(1.0)));
static_assert(noexcept(sin(1.0) + isdigit('1')));
static_assert(noexcept(vsnprintf(nullptr, 0, "x", __builtin_va_list())));
static_assert(!noexcept(printf("x")));
static_assert(!noexcept(malloc(1)));
static_assert(!noexcept(tan(1.0)));
static_assert(!noexcept(strncmp("", "", 1)));
static_assert(!noexcept(S::abs(1)));
static_assert(!noexcept(vfprintf(nullptr, "x", __builtin_va_list())));

void local() {
  extern double sqrt(double);
  static_assert(!noexcept(sqrt(1.0)));
}
