// [over.match.copy]/1, [over.match.class.deduct]/2: copy-initialization from
// one expression considers the guides callable with one argument only, so
// a guide with no parameters or with two required ones is neither a
// candidate nor noted when the deduction fails.
//@ expect: compile-fail
//@ std: c++20
//@ strict-diags
//@ diag: 22:3 error "no viable constructor or deduction guide for deduction of template arguments of 'G'"
//@ diag: 21:27 note "candidate template ignored: could not match 'G<T>' against 'int'"
//@ diag: 21:27 note "implicit deduction guide declared as 'template <class T> G(G<T>) -> G<T>'"

template <class T> struct W {
  W();
  W(T);
  W(T, T);
};
W w = 1;
W w2(1, 2);

template <class T> struct B { B(T); };
template <class T> struct G : B<T> { G(); G(int, T); };
G g = 1;
