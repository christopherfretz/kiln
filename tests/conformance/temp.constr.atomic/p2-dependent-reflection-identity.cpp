// [temp.constr.atomic]/2, [temp.over.link]/5: a dependent reflection in a
// parameter mapping is equivalent to the same reflection written in
// another template, so the overload adding a conjunct is more
// constrained ([temp.constr.order]): `^^T*`, a class template
// specialization `^^A<T>`, an alias template specialization `^^AT<T>`, a
// dependent type `^^typename A<T>::type`, a dependent qualified name
// `^^A<T>::x`, a template template parameter `^^TT` and `^^TT<T>`.
// Compared with GCC 16.1 (clang has no reflection). WP-821.
//@ expect: compile-ok
//@ oracle: gcc
//@ args: -freflection
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
using info = decltype(^^int);
template <info R> concept Q = R != info{};
template <class T> struct A { using type = T; static constexpr int x = 1; };
template <class T> using AT = A<T>;
template <class T> struct B {};

template <class T> requires Q<^^T*> constexpr int f1(T) { return 1; }
template <class T> requires Q<^^T*> && true constexpr int f1(T) { return 2; }
static_assert(f1(0) == 2);

template <class T> requires Q<^^A<T>> constexpr int f2(T) { return 1; }
template <class T> requires Q<^^A<T>> && true constexpr int f2(T) { return 2; }
static_assert(f2(0) == 2);

template <class T> requires Q<^^AT<T>> constexpr int f3(T) { return 1; }
template <class T> requires Q<^^AT<T>> && true constexpr int f3(T) { return 2; }
static_assert(f3(0) == 2);

template <class T> requires Q<^^typename A<T>::type> constexpr int f4(T) { return 1; }
template <class T> requires Q<^^typename A<T>::type> && true constexpr int f4(T) { return 2; }
static_assert(f4(0) == 2);

template <class T> requires Q<^^A<T>::x> constexpr int f5(T) { return 1; }
template <class T> requires Q<^^A<T>::x> && true constexpr int f5(T) { return 2; }
static_assert(f5(0) == 2);

template <template <class> class TT, class T> requires Q<^^TT> constexpr int f6(TT<T>) { return 1; }
template <template <class> class TT, class T> requires Q<^^TT> && true constexpr int f6(TT<T>) { return 2; }
static_assert(f6(B<int>{}) == 2);

template <template <class> class TT, class T> requires Q<^^TT<T>> constexpr int f7(TT<T>) { return 1; }
template <template <class> class TT, class T> requires Q<^^TT<T>> && true constexpr int f7(TT<T>) { return 2; }
static_assert(f7(B<int>{}) == 2);
