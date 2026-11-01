// [over.match.best.general]/2.13: of two otherwise equally good implicit
// guides, the one generated from a non-template constructor beats the
// one generated from a constructor template; /2.11 still prefers a
// user-declared guide over both.
//@ strict-diags
template<class T> struct S {
  S(T);
  template<class U = int> S(T);
};
S s(1);
static_assert(__is_same(decltype(s), S<int>));
template<class T> struct R {
  template<class U = int> R(T);
  R(T);
};
R r(1.0);
static_assert(__is_same(decltype(r), R<double>));
template<class T> struct G {
  template<class U = int> G(T);
};
template<class T> G(T) -> G<long>;
G g(1);
static_assert(__is_same(decltype(g), G<long>));
