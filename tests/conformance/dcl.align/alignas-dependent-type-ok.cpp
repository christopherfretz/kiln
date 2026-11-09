// [dcl.align]/3: valid twin of alignas-dependent-incomplete.cpp; a dependent
// alignas(type-id) with complete type arguments takes their alignment (WP-957).
//@ std: c++20
struct alignas(16) Big { char c; };
template <class T> struct alignas(T) A { char c; };
static_assert(alignof(A<Big>) == 16);
static_assert(alignof(A<char>) == 1);
template <class... T> struct alignas(T...) P { char c; };
static_assert(alignof(P<char, Big>) == 16);
template <class T> int f() { alignas(T) char x = 0; return x; }
template int f<Big>();
