//@ expect: compile-fail
//@ std: c++20
//@ strict-diags
//@ diag: 26:3 error "deduced type 'snd' does not satisfy 'big_env'"
//@ diag: 17:19 note "because 'sizeof(char) == 2' (1 == 2) evaluated to false"
// [dcl.type.auto.deduct]/2, [temp.arg.general]/9: `C auto` checks
// `C<T>` completed with C's default template arguments, also when C's
// constraint forwards the default into a variadic concept.
template <class S, class... E>
concept sender_in = sizeof...(E) == 1;
struct env {};
template <class S, class E = env>
concept next_sender = sender_in<S, E>;
template <class S, class E = env>
concept small_env = sizeof(E) == 1;
template <class S, class E = char>
concept big_env = sizeof(E) == 2;
struct snd {};
small_env auto g() { return snd{}; }
next_sender auto f() { return snd{}; }
void h(next_sender auto) {}
void k() {
  next_sender auto x = snd{};
  small_env auto y = x;
  h(y);
  big_env auto z = x;
}
