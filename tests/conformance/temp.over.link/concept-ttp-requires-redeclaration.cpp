// [temp.over.link]/5-6 (P2841R7): two function template declarations whose
// requires-clauses `C<T>` and `D<U>` name the corresponding concept
// template parameters are equivalent, so the second redeclares (and
// defines) the first and the call is not ambiguous. clang 22 keeps them as
// two candidates and reports the call ambiguous; kiln follows the wording
// (spec/sema-templates.md §6.8). WP-746.
//@ expect: run
//@ strict-diags
//@ skip(clang): clang 22 treats the renamed concept template parameter's requires-clause as a different declaration
//@ requires: clang>=22
//@ oracle: none
template <class T> concept Int = __is_same(T, int);
template <template <class> concept C, class T> constexpr int f(T) requires C<T>;
template <template <class> concept D, class U> constexpr int f(U) requires D<U> { return 1; }
static_assert(f<Int>(1) == 1);
template <template <class> concept... Cs, class T> int k(T) requires (Cs<T> && ...);
template <template <class> concept... Ds, class U> int k(U) requires (Ds<U> && ...) { return 0; }
int main() { return k<Int, Int>(1) + f<Int>(1) - 1; }
