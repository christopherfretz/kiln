// The GNU conversion that adds `noreturn` to a function pointer
// (gnu-noreturn-add-run.cpp) applies to implicit conversions only:
// dropping `noreturn` stays silent, a composite pointer type
// ([expr.type]/4, a comparison or a conditional) is the type without
// `noreturn`, as in clang, and a C-style cast reinterprets silently.
// WP-1551.
//@ expect: compile-ok
//@ strict-diags
typedef void (*fp)(int);
typedef void (*nfp)(int) __attribute__((noreturn));
void plain(int);
__attribute__((noreturn)) void dies(int);
[[noreturn]] void std_dies(int);
template<class A, class B> struct same { static const bool value = false; };
template<class A> struct same<A, A> { static const bool value = true; };
nfp a = dies;
fp b = dies;
fp c = a;
fp d = std_dies;
nfp e = (nfp)plain;
int k;
static_assert(same<decltype(k ? a : b), fp>::value, "");
static_assert(same<decltype(k ? b : a), fp>::value, "");
bool eq1 = a == b;
bool eq2 = a == plain;
bool eq3 = b != a;
