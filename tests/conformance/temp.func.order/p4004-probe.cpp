// P4004R1 (a DR, reconsidering CWG1395): partial ordering of a trailing
// function parameter pack against no parameter, and of class template
// partial specializations with a trailing pack, keep the long-standing
// behaviour (clang since 2.7, GCC 16.1).
//@ expect: compile-ok
//@ std: c++26
//@ strict-diags
template <class T> constexpr int f(T) { return 1; }
template <class T, class... U> constexpr int f(T, U...) { return 2; }
static_assert(f(1) == 1);
static_assert(f(1, 2) == 2);
template <class... T> constexpr int h(T...) { return 1; }
template <class T, class... U> constexpr int h(T, U...) { return 2; }
static_assert(h(1) == 2);
static_assert(h() == 1);
template <class T> struct A;
template <class T, class... U> struct A<void(T, U...)> { static constexpr int v = 2; };
template <class T> struct A<void(T)> { static constexpr int v = 1; };
static_assert(A<void(int)>::v == 1);
static_assert(A<void(int, int)>::v == 2);
