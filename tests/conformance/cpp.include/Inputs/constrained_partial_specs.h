// Constrained partial specializations (WP-1342): requires-clauses that
// subsume one another, a type-constraint on a parameter, and a
// requires-expression in the clause.
#ifndef CONSTRAINED_PARTIAL_SPECS_H
#define CONSTRAINED_PARTIAL_SPECS_H
namespace cps {
template <class T> constexpr bool is_ref_v = false;
template <class T> constexpr bool is_ref_v<T &> = true;
template <class T> concept ref = is_ref_v<T>;
template <class T> concept small = sizeof(T) <= 4;
template <class T> concept has_type = requires { typename T::type; };
template <class A, class B> struct pick { static constexpr int value = 0; };
template <class A, class B>
  requires ref<A> && has_type<B>
struct pick<A, B> { static constexpr int value = 1; };
template <class A, class B>
  requires ref<A> && has_type<B> && small<B>
struct pick<A, B> { static constexpr int value = 2; };
template <small A, class B> struct pick<A *, B> {
  static constexpr int value = 3;
};
template <class A, class B>
  requires is_ref_v<A> && requires { typename B::other; }
struct pick<A *, B> { static constexpr int value = 4; };
template <class T> constexpr int kind = 0;
template <class T>
  requires small<T>
constexpr int kind<T> = 1;
} // namespace cps
#endif
