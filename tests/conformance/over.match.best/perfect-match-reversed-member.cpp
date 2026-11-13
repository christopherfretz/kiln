// [over.match.best], clang 22.1: a member `operator==` beside its own
// reversed form is a unique perfect match (the rewritten candidate of the
// same function does not make it ambiguous), so the function template is
// not deduced and its hard-error signature is not reported. kiln deduces
// it (the downstream report series treated the two as rival perfect
// candidates); BACKLOG WP-635.
//@ expect: compile-ok
//@ requires: clang>=22
//@ strict-diags
//@ xfail(kiln): a member operator== beside its own reversed form is a unique perfect match in clang 22.1 (WP-635)
template <class T> struct bad {
  static_assert(sizeof(T) == 0, "instantiated");
  typedef int type;
};
struct Q {
  bool operator==(const Q &) const;
  template <class T> typename bad<T>::type operator==(const T &) const;
};
void use() {
  const Q q;
  (void)(q == q);
}
