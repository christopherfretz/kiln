// [temp.constr.fold], [temp.constr.order]/1 (P2963R3): a fold `(E && ...)`
// or `(E || ...)` normalizes to a fold expanded constraint, which subsumes
// another when both expand the same pack with the same operator and its
// constraint subsumes the other's; a left fold and a right fold are the
// same constraint, and `(E && ... && I)` is `(E && ...) && I`. Different
// operators or packs do not subsume (spec/sema-templates.md §8.5). WP-771.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 44:14 error "call to 'n' is ambiguous"
//@ diag: 38:62 note "candidate function [with Ts = <int>]"
//@ diag: 39:62 note "candidate function [with Ts = <int>]"
//@ diag: 44:20 error "call to 'p' is ambiguous"
//@ diag: 42:75 note "candidate function [with Ts = <int>, Us = <int>]"
//@ diag: 43:75 note "candidate function [with Ts = <int>, Us = <int>]"
template <class T> concept A = sizeof(T) >= 1;
template <class T> concept B = A<T> && __is_integral(T);
template <class... Ts> requires (A<Ts> && ...) constexpr int f(Ts...) { return 1; }
template <class... Ts> requires (B<Ts> && ...) constexpr int f(Ts...) { return 2; }
static_assert(f(1, 2) == 2);
static_assert(f(1.0, 2) == 1);
static_assert(f() == 2);
template <class... Ts> requires (A<Ts> || ...) constexpr int g(Ts...) { return 1; }
template <class... Ts> requires (B<Ts> || ...) constexpr int g(Ts...) { return 2; }
static_assert(g(1, 2.0) == 2);
// left fold same as right fold
template <class... Ts> requires (... && A<Ts>) constexpr int h(Ts...) { return 1; }
template <class... Ts> requires (B<Ts> && ...) constexpr int h(Ts...) { return 2; }
static_assert(h(1) == 2);
// with init
template <class T, class... Ts> requires (A<Ts> && ... && A<T>) constexpr int k(T, Ts...) { return 1; }
template <class T, class... Ts> requires (B<Ts> && ... && B<T>) constexpr int k(T, Ts...) { return 2; }
static_assert(k(1, 2) == 2);
// mixed && and fold
template <class... Ts> requires (A<Ts> && ...) constexpr int m(Ts...) { return 1; }
template <class... Ts> requires (A<Ts> && ...) && (sizeof...(Ts) > 1) constexpr int m(Ts...) { return 2; }
static_assert(m(1, 2) == 2);
// different op: not subsumed
template <class... Ts> requires (A<Ts> && ...) constexpr int n(Ts...) { return 1; }
template <class... Ts> requires (B<Ts> || ...) constexpr int n(Ts...) { return 2; }
// different pack -> not compatible
template <class... Ts> struct L {};
template <class... Ts, class... Us> requires (A<Ts> && ...) constexpr int p(L<Ts...>, L<Us...>) { return 1; }
template <class... Ts, class... Us> requires (B<Us> && ...) constexpr int p(L<Ts...>, L<Us...>) { return 2; }
int main() { n(1); p(L<int>{}, L<int>{}); }
