// [temp.constr.order]/3-4 (P2841R7): associated constraints that contain a
// concept-dependent constraint (a concept-id or fold whose concept is a
// concept template parameter, [temp.constr.normal]/1.4.1) are not eligible
// for subsumption, so neither candidate is more constrained and the calls
// are ambiguous, with clang 22's "similar constraint" notes. `f` is
// [temp.constr.order] Example 3, whose comment selects #1; the normative
// ¶4 makes it ambiguous, as clang 22 does, and kiln follows ¶4
// (spec/sema-templates.md §8.5). Constraints without one (`q`) still order
// normally. WP-746.
//@ expect: compile-fail
//@ strict-diags
//@ requires: clang>=22
//@ diag: 33:9 error "call to 'f' is ambiguous"
//@ diag: 30:15 note "candidate function [with X = A, T = int]"
//@ diag: 32:15 note "candidate function [with X = A, T = int]"
//@ diag: 38:9 error "call to 'n' is ambiguous"
//@ diag: 35:15 note "candidate function [with CT = A, T = int]"
//@ diag: 37:15 note "candidate function [with CT = A, T = int]"
//@ diag: 34:64 note "similar constraint expressions not considered equivalent; constraint expressions cannot be considered equivalent unless they originate from the same concept"
//@ diag: 36:64 note "similar constraint expression here"
//@ diag: 43:9 error "call to 'm' is ambiguous"
//@ diag: 40:15 note "candidate function [with Cs = <A>, T = int]"
//@ diag: 42:15 note "candidate function [with Cs = <A>, T = int]"
//@ diag: 39:68 note "similar constraint expressions not considered equivalent; constraint expressions cannot be considered equivalent unless they originate from the same concept"
//@ diag: 41:68 note "similar constraint expression here"
template <typename T> concept A = true;
template <typename T> concept C = true;
template <template <typename> concept CT, typename T> struct S {};
template <template <typename> concept X, typename T>
constexpr int f(S<X, T>) requires A<T> { return 1; }
template <template <typename> concept X, typename T>
constexpr int f(S<X, T>) requires X<T> { return 2; }
int r = f(S<A, int>{});
template <template <typename> concept CT, typename T> requires CT<T>
constexpr int n(T) { return 1; }
template <template <typename> concept CT, typename T> requires CT<T> && A<T>
constexpr int n(T) { return 2; }
int y = n<A>(1);
template <template <typename> concept... Cs, typename T> requires (Cs<T> && ...)
constexpr int m(T) { return 1; }
template <template <typename> concept... Cs, typename T> requires (Cs<T> && ...) && true
constexpr int m(T) { return 2; }
int x = m<A>(1);
template <template <typename> concept CT, typename T> requires A<T>
constexpr int q(T) { return 1; }
template <template <typename> concept CT, typename T> requires A<T> && C<T>
constexpr int q(T) { return 2; }
static_assert(q<C>(1) == 2);
