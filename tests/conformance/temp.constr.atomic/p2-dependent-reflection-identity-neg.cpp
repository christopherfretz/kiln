// [temp.constr.atomic]/2, [temp.over.link]/5: dependent reflections that
// are not equivalent map to different atoms and the call is ambiguous: an
// alias template specialization `^^AT<T>` is not `^^A<T>` (a type
// reflection keeps a top-level alias, [meta.reflection]), `^^A<T>::x` is
// not `^^A<T>::y`, `^^T*` is not `^^const T*`. Compared with GCC 16.1
// (clang has no reflection). WP-821.
//@ expect: compile-fail
//@ oracle: gcc
//@ args: -freflection
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
//@ diag: 28:15 error "call to 'n1' is ambiguous"
//@ diag: 26:54 note "candidate function [with T = int]"
//@ diag: 27:61 note "candidate function [with T = int]"
//@ diag: 32:15 error "call to 'n2' is ambiguous"
//@ diag: 30:56 note "candidate function [with T = int]"
//@ diag: 31:64 note "candidate function [with T = int]"
//@ diag: 36:15 error "call to 'n3' is ambiguous"
//@ diag: 34:51 note "candidate function [with T = int]"
//@ diag: 35:65 note "candidate function [with T = int]"
using info = decltype(^^int);
template <info R> concept Q = R != info{};
template <class T> struct A { using type = T; static constexpr int x = 1; static constexpr int y = 2; };
template <class T> using AT = A<T>;

template <class T> requires Q<^^AT<T>> constexpr int n1(T) { return 1; }
template <class T> requires Q<^^A<T>> && true constexpr int n1(T) { return 2; }
static_assert(n1(0) == 2);

template <class T> requires Q<^^A<T>::x> constexpr int n2(T) { return 1; }
template <class T> requires Q<^^A<T>::y> && true constexpr int n2(T) { return 2; }
static_assert(n2(0) == 2);

template <class T> requires Q<^^T*> constexpr int n3(T) { return 1; }
template <class T> requires Q<^^const T*> && true constexpr int n3(T) { return 2; }
static_assert(n3(0) == 2);
