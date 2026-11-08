// [dcl.attr.grammar]/4, /6 and GNU `__attribute__`: an attribute name
// written `__x__` names the attribute `x` (clang `normalizeAttrName`,
// WP-678; spec/sema.md §4.5) for every attribute, in GNU syntax and in
// `[[...]]` with no scope or the `gnu` or `clang` scope. Exactly one
// leading and one trailing `__` are removed, and only when both are
// present: `__noinline`, `noinline__` and `____noinline____` are unknown,
// while `__const` and `gnu::__const` are clang's own spellings of `const`.
// An attribute in any other scope keeps its name.
//@ expect: compile-ok
//@ strict-diags
//@ diag: 38:16 warning "'__noinline' ignored"
//@ diag: 39:16 warning "'noinline__' ignored"
//@ diag: 40:16 warning "'____noinline____' ignored"
//@ diag: 41:16 warning "'__unused' ignored"
//@ diag: 42:3 warning "__noinline' ignored"
//@ diag: 43:3 warning "optnone__"
//@ diag: 44:3 warning "__noinline__' ignored"

__attribute__((__no_sanitize__("address"))) void s1() {}
__attribute__((__no_sanitize__("address", "undefined"))) void s2() {}
__attribute__((__noinline__, __cold__)) void n1() {}
__attribute__((__always_inline__)) inline void n2() {}
__attribute__((__visibility__("default"), __used__)) void n3() {}
__attribute__((__format__(__printf__, 1, 2))) void n4(const char *, ...);
__attribute__((__const)) int c1();
__attribute__((__const__)) int c2();
[[gnu::__const]] int c3();
[[gnu::__always_inline__]] inline void a1() {}
[[__gnu__::__noinline__]] void a2() {}
[[clang::__optnone__]] void a3() {}
struct __attribute__((__packed__)) P { char c; int i; };
static_assert(sizeof(P) == sizeof(int) + 1);
struct __attribute__((__aligned__(16))) Q { int i; };
static_assert(alignof(Q) == 16);
typedef int V __attribute__((__vector_size__(16)));
static_assert(sizeof(V) == 16);

__attribute__((__noinline)) void u1();
__attribute__((noinline__)) void u2();
__attribute__((____noinline____)) void u3();
__attribute__((__unused)) void u4();
[[gnu::__noinline]] void u5();
[[clang::____optnone____]] void u6();
[[msvc::__noinline__]] void u7();

static_assert(__has_attribute(__noinline__));
static_assert(__has_attribute(__const));
static_assert(!__has_attribute(__noinline));
static_assert(!__has_attribute(noinline__));
static_assert(!__has_attribute(____noinline____));
static_assert(__has_cpp_attribute(gnu::__const));
static_assert(__has_cpp_attribute(clang::__noinline__));
static_assert(!__has_cpp_attribute(clang::____noinline____));
static_assert(!__has_cpp_attribute(gnu::__noinline));
