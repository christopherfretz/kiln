// [temp.deduct.general]/2-3, /8, [temp.arg.explicit]/9: the explicit
// arguments are substituted throughout the function type, into a nested
// expansion of the trailing pack's pattern too. `h<int>` forms
// `int::type` in `typename T::type` beside the nested `Ts...`, and `k<int>`
// in `typename Us::type...` over the explicitly given `Us = {int}`: both
// are deduction failures, although the outer `Ts` deduces empty. GCC 16
// accepts `k<int>()` (it forms the nested expansion only per element of
// `Ts`); the standard substitutes `Us` before deduction, as clang does.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 18:12 error "no matching function for call to 'h'"
//@ diag: 16:38 note "candidate template ignored: substitution failure [with T = int]: type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 19:12 error "no matching function for call to 'k'"
//@ diag: 17:42 note "candidate template ignored: substitution failure [with Us = <int>]: type 'int' cannot be used prior to '::' because it has no members"
template <class...> struct L {};
template <class T, class... Ts> void h(L<Ts, typename T::type, Ts...>...) {}
template <class... Us, class... Ts> void k(L<Ts, typename Us::type...>...) {}
void g() { h<int>(); }
void f() { k<int>(); }
