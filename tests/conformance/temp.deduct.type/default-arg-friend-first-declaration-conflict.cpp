// [temp.deduct.type]/9, [temp.arg.general]/9, [temp.deduct.call]/1: a
// class template-id completed with its defaults (first declared as a friend
// of a class template) still diagnoses a genuine conflict: the default's
// substituted argument must match `A`'s, and a parameter deduced from two
// places must agree. clang 19's wording (WP-286).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 26:3 error "no matching function for call to 'h'"
//@ diag: 21:24 note "candidate template ignored: deduced conflicting types for parameter 'C' ('int' vs. 'char')"
//@ diag: 27:3 error "no matching function for call to 'm'"
//@ diag: 22:24 note "candidate template ignored: could not match 'traits<int>' against 'traits<char>'"
//@ diag: 28:3 error "no matching function for call to 'n'"
//@ diag: 23:24 note "candidate template ignored: deduced conflicting types for parameter 'C' ('char' vs. 'int')"
template <class C> struct traits {};
template <class I> class wrap {
  template <class C, class Tr> friend class sv;
};
template <class C, class Tr = traits<C>> class sv;
template <class C, class Tr> class sv {};

template <class C> int h(C, sv<C>);
template <class C> int m(sv<C, traits<int>>);
template <class C> int n(sv<C>, sv<C>);

void t() {
  h(1, sv<char>{});
  m(sv<char>{});
  n(sv<char>{}, sv<int>{});
}
