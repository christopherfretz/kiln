// GNU `selectany` (WP-809; clang `SelectAnyAttr`): a variable definition
// is `weak_odr` in its own comdat, a dynamic initializer is unordered in
// that comdat (with its guard, and the variable in `llvm.used`), an
// inline variable or template instantiation keeps `linkonce_odr`, and
// `weak` stays `weak` (now in a comdat). A `selectany` (or `weak`)
// `thread_local` is accessed through its wrapper even when constant
// initialized (clang `isEmittedWithConstantInitializer`). On a type, a
// typedef, a parameter or a field it has no effect.
__attribute__((selectany)) int a = 1;
[[gnu::selectany]] int b = 2;
__attribute__((selectany)) extern const int d = 4;
struct S { int x; };
__attribute__((selectany)) S s = {5};
__attribute__((selectany)) int e;
struct T { static int m; };
__attribute__((selectany)) int T::m = 7;
extern int g;
__attribute__((selectany)) int g = 4;
int dyn();
__attribute__((selectany)) int q = dyn();
struct C { C(); ~C(); };
__attribute__((selectany)) C cc;
__attribute__((selectany)) thread_local int tl = 1;
__attribute__((selectany)) thread_local int tl2 = dyn();
__attribute__((selectany)) inline int iv = 1;
struct F { int m __attribute__((selectany)); static int s __attribute__((selectany)); };
int F::s = 1;
__attribute__((selectany)) extern constexpr int ce2 = 1;
namespace N { __attribute__((selectany)) int n = 1; }
int arr[2] __attribute__((selectany)) = {1, 2};
__attribute__((selectany)) const int *cp = nullptr;
int b4 __attribute__((selectany)) = 1;
template <class X> struct TS { static int v; };
template <class X> __attribute__((selectany)) int TS<X>::v = 2;
__attribute__((selectany)) int x1 = 1, x2 = 2;
extern "C" __attribute__((selectany)) int ec = 3;
__attribute__((weak, selectany)) int w = 3;
struct __attribute__((selectany)) X { int a; };
typedef int TI __attribute__((selectany));
void p(int x __attribute__((selectany)));
int use() { return a + b + d + s.x + e + T::m + g + q + tl + tl2 + iv + F::s + ce2 + N::n + arr[0] + *cp + b4 + TS<int>::v + x1 + ec + w; }
