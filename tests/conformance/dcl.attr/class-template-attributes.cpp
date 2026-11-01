// [dcl.attr.grammar]/5, [dcl.align]: attributes of a class template's definition appertain to each
// specialization (GNU packed, alignas with a dependent operand), and to member classes
//@ expect: compile-ok
//@ strict-diags
template <class T> struct __attribute__((packed)) P { char c; T x; };
static_assert(sizeof(P<int>) == 5);
template <class T> struct Q { struct __attribute__((packed)) I { char c; T x; }; };
static_assert(sizeof(Q<int>::I) == 5);
template <class T> struct alignas(16) R { T x; };
static_assert(alignof(R<int>) == 16);
template <class T> struct alignas(T) S { char c; };
static_assert(alignof(S<long>) == alignof(long) && sizeof(S<double>) == sizeof(double));
template <int N> struct alignas(N) U { char c; };
static_assert(alignof(U<32>) == 32);
