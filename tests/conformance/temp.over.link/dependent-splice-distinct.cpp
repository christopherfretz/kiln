// [temp.over.link]/5, [temp.constr.order], P2996 [temp.type]: dependent
// splices that differ in their operand, in a splice template-id's
// arguments, or around a member splice's parentheses are not equivalent,
// so neither overload is more constrained (or more specialized) and each
// call is ambiguous. Compared with GCC 16.1 (clang has no reflection).
// WP-884.
//@ expect: compile-fail
//@ oracle: gcc
//@ args: -freflection
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
//@ diag: 29:10 error "call to 'h1' is ambiguous"
//@ diag: 27:61 note "candidate function [with R = ^^one, Q = ^^one]"
//@ diag: 28:69 note "candidate function [with R = ^^one, Q = ^^one]"
//@ diag: 33:10 error "call to 'h2' is ambiguous"
//@ diag: 31:67 note "candidate function [with R = ^^vt]"
//@ diag: 32:76 note "candidate function [with R = ^^vt]"
//@ diag: 37:10 error "call to 'h3' is ambiguous"
//@ diag: 35:24 note "candidate function [with R = ^^S::m]"
//@ diag: 36:24 note "candidate function [with R = ^^S::m]"
using info = decltype(^^int);
template <int N> concept P = N > 0;
constexpr int one = 1;
struct S { int m; };
template <class T> constexpr int vt = sizeof(T);

template <info R, info Q> requires P<([:R:])> constexpr int h1() { return 1; }
template <info R, info Q> requires P<([:Q:])> && true constexpr int h1() { return 2; }
int a1 = h1<^^one, ^^one>();

template <info R> requires P<(template [:R:]<int>)> constexpr int h2() { return 1; }
template <info R> requires P<(template [:R:]<long>)> && true constexpr int h2() { return 2; }
int a2 = h2<^^vt>();

template <info R> auto h3(S s) -> decltype(s.[:R:]) { return 1; }
template <info R> auto h3(S s) -> decltype((s.[:R:])) { return 2; }
int a3 = h3<^^S::m>(S{0});
