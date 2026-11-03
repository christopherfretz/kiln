// [temp.arg.nontype]/2, [temp.inst]: a non-type template parameter of type
// `_BitInt(N)` with N > 128 (or a placeholder deduced as one) is replaced in
// the instantiation by a literal of the argument's exact value, which
// constant evaluation reads limb by limb. clang is the oracle; x86-64 only
// (aarch64's `__BITINT_MAXWIDTH__` is 128).
//@ args: --target=x86_64-unknown-linux-gnu
using W = _BitInt(200);
using U = unsigned _BitInt(300);
template <W V> constexpr W f() { return V + 1; }
static_assert(f<(W(1) << 160)>() == (W(1) << 160) + 1);
static_assert(f<-1>() == 0);
template <W V> struct S { static constexpr W v = V; };
static_assert(S<-(W(1) << 190)>::v == -(W(1) << 190));
template <auto V> constexpr auto h = V;
static_assert(h<U(1) << 299> == U(1) << 299);
static_assert(h<~U(0)> == ~U(0));
