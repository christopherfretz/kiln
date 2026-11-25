// [temp.deduct.general]/2-3, /8, [temp.arg.explicit]/9: a parameter type
// that names the explicitly given pack `Ts` is kept for deduction (which
// may extend `Ts`), but the explicit arguments are still substituted into
// it: the expansion over the open pack `Us` forms `int::type` with
// `T = int`, a deduction failure although `Us` deduces empty.
//@ expect: compile-fail
//@ strict-diags
//@ oracle: gcc
//@ diag: 14:12 error "no matching function for call to 'f'"
//@ diag: 13:51 note "candidate template ignored: substitution failure [with T = int, Ts = <char>]: type 'int' cannot be used prior to '::' because it has no members"
template <class...> struct tup {};
template <class...> struct L {};
template <class T, class... Ts, class... Us> void f(tup<Ts..., L<Us, typename T::type>...>) {}
void g() { f<int, char>(tup<char>{}); }
