// [temp.param]/7.1, [temp.arg.nontype]/1, [temp.type]/2: a GNU
// `_Complex` type is a structural type (a scalar extension, as clang), so
// it is a valid non-type template parameter type. The substituted
// argument is a prvalue of its value; two arguments are the same when
// their parts are identical (so `-0.0` and `+0.0` differ). The mangled
// names match clang (tests/codegen/nttp_complex.cpp). WP-810.
//@ std: c++20
//@ expect: run
//@ strict-diags
//@ stdout: 1 2 7 2 8 1 0 1 -3
#include <cstdio>
typedef _Complex float cf;
typedef _Complex double cd;
typedef _Complex int ci;
template <cf C> struct A { static constexpr cf v = C; };
template <cd D> int f() { return __real__ D + __imag__ D; }
constexpr cf c1 = {1.0f, 2.0f};
template <ci I> int g() { return __imag__ I; }
template <auto X> int h() { return sizeof(X); }
template <cd D> constexpr cd twice() { return D + D; }
template <class T, class U> constexpr bool same = false;
template <class T> constexpr bool same<T, T> = true;
static_assert(same<A<c1>, A<cf{1.0f, 2.0f}>>);
static_assert(!same<A<cf{0.0f, 0.0f}>, A<cf{-0.0f, 0.0f}>>);
static_assert(__real__ twice<cd{1.5, -2.0}>() == 3.0);
static_assert(__imag__ A<c1>::v == 2.0f);
int main() {
  cf local = A<c1>::v;
  ci n = ci{-3, 4};
  printf("%g %g %d %d %d %d %d %d %d\n", __real__ local, __imag__ A<c1>::v,
         f<cd{3.0, 4.0}>(), g<ci{1, 2}>(), h<cf{1, 0}>(),
         same<A<c1>, A<cf{1.0f, 2.0f}>>, same<A<cf{1.0f, 0.0f}>, A<c1>>,
         (int)__real__ twice<cd{0.5, 0.0}>(),
         __real__ n);
}
