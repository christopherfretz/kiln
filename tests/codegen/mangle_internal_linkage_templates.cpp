// [basic.link]/4, Itanium ABI <unscoped-name> ::= L <source-name>: a
// specialization of a template with internal linkage (a `static` function
// or variable template, one in an unnamed namespace, a closure-typed
// variable template) carries the `L` prefix of its template, as clang
// emits it (`_ZL1hIiEiT_`, `_ZL2laIiE`). WP-603 (the `L` was missing for
// specializations). A closure initializing such a variable template is
// internal too (clang `getLVForClosureType`; it was `linkonce_odr`).
//
// [temp.arg.nontype], Itanium ABI <template-arg>: a pointer or reference
// argument naming a whole object whose type differs from the parameter's
// pointee (`int g` for `const int*`) is mangled through a conversion,
// `XcvPKiadL_Z1gEE`. WP-933. Compared with clang 19 and 22 (ir_vs_clang).
template <class T> static int h(T t) { return t; }
namespace { template <class T> int g(T t) { return t; } }
namespace n { template <class T> static int k(T t) { return t; } }
template <class T> static T vt = T(1);
namespace { template <class T> T vu = T(2); }
namespace { struct A { template <class T> int f(T t) { return t; } }; }
template <class T> static int h2(T) { return 0; }
template <> int h2<long>(long) { return 1; }
template <class T> static auto la = [](T x) { return x; };
template <int (*F)(int)> int call(int x) { return F(x); }

template <const int* P> int deref() { return *P; }
template <const int& R> int ref() { return R; }
template <const volatile int* P> int vol() { return *P; }
int arr[3];
int gi = 1;
static int si = 2;
const int cg = 3;
namespace { int an = 5; }

int use() {
  return h(1) + g(2) + n::k(3) + vt<int> + vu<long> + A().f(5) + h2(1L) + h2('c') +
         la<int>(3) + call<h<int>>(1) + deref<&vt<int>>() + deref<&gi>() + deref<&si>() +
         deref<&cg>() + deref<&an>() + deref<arr>() + ref<gi>() + ref<cg>() + vol<&gi>() +
         vol<&cg>();
}
