// [temp.constr.normal]/1.5.3 with [temp.constr.atomic]/1 (P2841R7): when a
// fold over a concept template parameter pack expands, an atom of its init
// or pattern that names no concept parameter has a parameter mapping
// without the pack, so `true` of all_t<T, A> and all_t<T, B> (and
// `sizeof(T) > 0` of each element of all_s) is one atomic constraint and the
// stronger concepts' overload subsumes. clang 22 keeps concept-pack folds
// whole and reports both calls ambiguous; kiln follows the wording
// (spec/sema-templates.md §8.2 "Wording over clang"). WP-771.
//@ expect: compile-ok
//@ strict-diags
//@ skip(clang): clang 22 does not expand concept-pack folds in subsumption
//@ requires: clang>=22
template <class T> concept A = sizeof(T) >= 1;
template <class T> concept B = A<T> && __is_integral(T);

// the fold's init names no concept parameter
template <class T, template <class> concept... Cs>
concept all_t = (Cs<T> && ... && true);
template <class T> requires all_t<T, A> constexpr int g(T) { return 1; }
template <class T> requires all_t<T, B> constexpr int g(T) { return 2; }
static_assert(g(1) == 2);

// an atom of the pattern that names no concept parameter
template <class T, template <class> concept... Cs>
concept all_s = ((Cs<T> && sizeof(T) > 0) && ...);
template <class T> requires all_s<T, A> constexpr int h(T) { return 1; }
template <class T> requires all_s<T, A, B> constexpr int h(T) { return 2; }
static_assert(h(1) == 2);
