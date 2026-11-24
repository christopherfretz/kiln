// [temp.spec.partial.match]/1, [temp.variadic]/7: a member of `S<int>` whose
// parameter is a pack expansion over the empty pack `Ts` expands to no
// parameters, so its pattern (`decltype(v<int, int>)`, `sizeof(X<int, int>)`)
// is never substituted and nothing in it is an error. kiln still probes
// the pattern with its diagnostics dropped; the ambiguity it meets there
// is remembered, and must not make the later plain uses of `X<int, int>`
// and `v<int, int>` quiet: each is diagnosed whichever comes first
// (WP-3283; spec/sema-templates.md §5.7.3 "Hard use of a silent failure").
// Here `S<int>` comes first; the sibling test has the plain uses first.
//@ expect: compile-fail
//@ strict-diags
//@ oracle: gcc
//@ diag: 31:13 error "ambiguous partial specializations of 'X<int, int>'"
//@ diag: 20:27 note "partial specialization matches [with T = int]"
//@ diag: 21:27 note "partial specialization matches [with U = int]"
//@ diag: 32:9 error "ambiguous partial specializations of 'v<int, int>'"
//@ diag: 23:34 note "partial specialization matches [with T = int]"
//@ diag: 24:34 note "partial specialization matches [with U = int]"
template <class T, class U> struct X;
template <class T> struct X<T, int> {};
template <class U> struct X<int, U> {};
template <class T, class U> constexpr int v = 0;
template <class T> constexpr int v<T, int> = 1;
template <class U> constexpr int v<int, U> = 2;
template <class...> struct L {};
template <class T, class... Ts> struct S {
  void h(L<Ts, decltype(v<T, int>)>...);
  void k(L<Ts, decltype(sizeof(X<T, int>))>...);
};
S<int> s;
X<int, int> x;
int y = v<int, int>;
