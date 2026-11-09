// [class.copy.assign]/1, /9: an explicit-object `operator=(this S &, const S &)`
// is a copy assignment operator and, defaulted, is trivial when the
// implicit one would be; a defaulted explicit-object move assignment
// suppresses the implicit copy assignment ([class.copy.assign]/2). WP-958.
//@ std: c++23
//@ requires: clang>=22
//@ expect: compile-ok
struct S {
  S &operator=(this S &, const S &) = default;
  int v;
};
static_assert(__is_trivially_assignable(S &, const S &));
struct M {
  M &operator=(this M &, M &&) = default;
  int v;
};
static_assert(__is_assignable(M &, M &&));
static_assert(!__is_assignable(M &, const M &));
