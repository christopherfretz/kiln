// A dependent __builtin_offsetof (dependent type or array index) is kept
// at the definition and computed on substitution, as clang's
// TransformOffsetOfExpr does; the offsetof macro form, nested and array
// designators, and a class template naming itself (WP-307).
//@ expect: run
typedef __SIZE_TYPE__ size_t;
#define offsetof(t, m) __builtin_offsetof(t, m)
struct P { int a; double b; int arr[4]; struct { char c; } in; };
template<class T> constexpr size_t off_b() { return __builtin_offsetof(T, b); }
template<class T, int N> constexpr size_t off_arr() { return __builtin_offsetof(T, arr[N]); }
template<class T> constexpr size_t off_in() { return offsetof(T, in.c); }
template<class T> struct H { static constexpr size_t v = __builtin_offsetof(T, b) + sizeof(T); };
static_assert(off_b<P>() == 8);
static_assert(off_arr<P, 2>() == 24);
static_assert(off_in<P>() == 32);
static_assert(H<P>::v == 8 + sizeof(P));

template<class T> struct S { int x, y; static constexpr size_t o = __builtin_offsetof(S, y); };
static_assert(S<int>::o == 4);
template<class T, int N> size_t off_i() { return __builtin_offsetof(P, arr[N]) + sizeof(T); }
int main() { return off_b<P>() == 8 && off_i<char, 3>() == 29 ? 0 : 1; }
