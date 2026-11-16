// [over.match.class.deduct]/3, [dcl.fct.spec]/4: a guide template with a
// value-dependent explicit-specifier is a candidate of copy-initialization
// until its specialization says it is explicit; with explicit(true) it is
// not viable there and the deduction fails.
//@ expect: compile-fail
//@ std: c++20
//@ strict-diags
//@ diag: 28:3 error "no viable constructor or deduction guide for deduction of template arguments of 'S'"
//@ diag: 21:22 note "candidate template ignored: couldn't infer template argument 'V'"
//@ diag: 21:22 note "implicit deduction guide declared as 'template <bool V, class T> S(T) -> S<V>'"
//@ diag: 20:26 note "candidate template ignored: could not match 'S' against 'Flag'"
//@ diag: 20:26 note "implicit deduction guide declared as 'template <bool V> S(S<V>) -> S<V>'"
//@ diag: 23:31 note "explicit deduction guide is not a candidate (explicit specifier evaluates to true)"

template <bool B> struct Flag {
  static constexpr bool value = B;
  constexpr operator bool() const { return B; }
};

template <bool V> struct S {
  template <class T> S(T);
};
template <bool B> explicit(B) S(Flag<B>) -> S<B>;

S ok = Flag<false>{};
static_assert(sizeof(ok) == sizeof(S<false>));
S direct(Flag<true>{});
S bad = Flag<true>{};
