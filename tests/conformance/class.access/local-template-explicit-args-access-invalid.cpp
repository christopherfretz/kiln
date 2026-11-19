// [class.access.general]/2, [temp.deduct.general]/8: the twin of
// local-template-explicit-args-access.cpp. The access context of a local
// template's signature substitution must not grant access outside it either:
// a generic lambda in a member function of `X` sees `X`'s private members,
// a later namespace-scope function does not. A private member named in a
// function template's signature stays a substitution failure (WP-1493).
// Reduced from real code (WP-1726).
//@ expect: compile-fail
//@ std: c++20
//@ strict-diags
//@ diag: 23:24 error "'p' is a private member of 'X'"
//@ diag: 17:7 note "implicitly declared private here"
//@ diag: 25:9 error "no matching function for call to 'sig'"
//@ diag: 24:25 note "candidate template ignored: substitution failure [with T = X]: 'p' is a private member of 'X'"

class X {
  int p = 1;
  void m() {
    auto l = []<class T>(T x) { return x; };
    (void)l.operator()<int>(1);
  }
};
int h(X &x) { return x.p; }
template <class T> auto sig(T t) -> decltype(t.p) { return t.p; }
int u = sig(X{});
