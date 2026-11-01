// [dcl.pre]/11 (C++26, P2741R3): the message of `static_assert` may be a
// constant expression with `size()` and `data()` members, evaluated only
// when the assertion fails. WP-185 feature probe.
//@ expect: compile-fail
//@ strict-diags
//@ xfail(kiln): static_assert accepts only a string literal message
//@ diag: 16:15 error "static assertion failed: kiln-msg"
struct Msg {
  const char* p;
  constexpr unsigned long size() const { unsigned long n = 0; while (p[n]) ++n; return n; }
  constexpr const char* data() const { return p; }
};
template <bool B> struct Check { static_assert(B, Msg{"unused"}); };
Check<true> ok;
static_assert(true, Msg{"never evaluated is fine"});
static_assert(false, Msg{"kiln-msg"});
