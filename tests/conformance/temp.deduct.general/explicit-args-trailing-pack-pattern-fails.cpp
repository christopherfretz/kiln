// [temp.deduct.general]/2-3, [temp.arg.explicit]/9: the explicitly
// specified `T = int` is substituted into the function type before
// deduction; the trailing function parameter pack keeps its expansion
// (`Ts` may still be deduced), but `typename T::type` in its pattern is
// substituted, fails, and so `h` is not viable, although `Ts` turns out
// empty: the final substitution, with `Ts` empty, never forms the
// pattern ([temp.variadic]/7). Found while checking the specialization
// memo (WP-3283).
//@ expect: compile-fail
//@ strict-diags
//@ oracle: gcc
//@ diag: 16:12 error "no matching function for call to 'h'"
//@ diag: 15:38 note "candidate template ignored: substitution failure [with T = int]: type 'int' cannot be used prior to '::' because it has no members"
template <class...> struct L {};
template <class T, class... Ts> void h(L<Ts, typename T::type>...) {}
void g() { h<int>(); }
