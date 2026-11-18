// GCC's __builtin_has_attribute(type-or-expression, attribute): whether the
// declaration or type the first operand names carries the attribute (with
// arguments: matching ones). A name asks its declaration (all of its
// declarations), then the declaration's type; another expression asks its
// type; a pointer's attribute is not its pointee's. Found running GCC's C++
// testsuite. WP-1611.
//@ std: c++20
//@ expect: compile-ok
//@ requires: clang>=22
//@ oracle: gcc
//@ skip(clang): clang 19/22 have no __builtin_has_attribute
//@ strict-diags
#define HAS(x, ...) __builtin_has_attribute(x, __VA_ARGS__)

typedef int Int16 __attribute__((aligned(16)));
struct __attribute__((aligned(32))) Big { char c; };
struct Plain { int i; };

__attribute__((aligned(8))) int a8;
Int16 *p16;
int plain;

void f() __attribute__((noreturn));
void f();
void g() __attribute__((constructor(200)));
void *h(int, int) __attribute__((alloc_size(1, 2), malloc));
void s() __attribute__((section(".text.s")));
void n(void *, void *) __attribute__((nonnull(2)));
void nall(void *) __attribute__((nonnull));
__attribute__((cold)) void c();
void c();

typedef float v4f __attribute__((vector_size(16)));
v4f vec;

// A name: its declaration first.
static_assert(HAS(a8, aligned));
static_assert(HAS(a8, aligned(8)));
static_assert(!HAS(a8, aligned(16)));
static_assert(!HAS(plain, aligned));
// A type: through its typedefs and its class.
static_assert(HAS(Int16, aligned(16)));
static_assert(!HAS(Int16, aligned(8)));
static_assert(HAS(Big, aligned));
static_assert(HAS(Big, __aligned__(32)));
static_assert(!HAS(Plain, aligned));
static_assert(!HAS(int, aligned));
// A pointer's attributes are not its pointee's.
static_assert(!HAS(p16, aligned));
static_assert(HAS(*p16, aligned(16)));
static_assert(!HAS(p16 + 1, aligned));
// Function attributes, on any declaration.
static_assert(HAS(f, noreturn));
static_assert(HAS(c, cold));
static_assert(!HAS(c, hot));
static_assert(HAS(g, constructor));
static_assert(HAS(g, constructor(200)));
static_assert(!HAS(g, constructor(201)));
static_assert(!HAS(g, destructor));
static_assert(HAS(h, malloc));
static_assert(HAS(h, alloc_size));
static_assert(HAS(h, alloc_size(1, 2)));
static_assert(HAS(s, section(".text.s")));
static_assert(!HAS(s, section(".text.t")));
static_assert(!HAS(n, nonnull));
static_assert(HAS(nall, nonnull));
static_assert(HAS(nall, nonnull(1)));
static_assert(HAS(n, nonnull(2)));
static_assert(!HAS(n, nonnull(1)));
// vector_size lives in the type.
static_assert(HAS(vec, vector_size));
static_assert(HAS(vec, vector_size(16)));
static_assert(!HAS(vec, vector_size(32)));
static_assert(!HAS(plain, vector_size));
// The operand is not evaluated; the result is a bool constant.
static_assert(__is_same(decltype(HAS(plain++, aligned)), bool));

int main() { return plain; }
