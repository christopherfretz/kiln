// [temp.deduct.funcaddr]/1, [temp.deduct.general]/2-3, /8: taking the
// address of `h<int>` substitutes the explicit `T = int` into the
// trailing pack's pattern before `Ts` is deduced from the target type;
// `int::type` fails, so no specialization matches `void ()`.
//@ expect: compile-fail
//@ strict-diags
//@ oracle: gcc
//@ diag: 12:16 error "address of overloaded function 'h' does not match required type 'void ()'"
//@ diag: 11:38 note "candidate template ignored: substitution failure [with T = int]: type 'int' cannot be used prior to '::' because it has no members"
template <class...> struct L {};
template <class T, class... Ts> void h(L<Ts, typename T::type>...) {}
void (*p)() = &h<int>;
