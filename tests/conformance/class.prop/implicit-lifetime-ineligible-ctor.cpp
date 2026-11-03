// [class.prop]/9, [special]/6: a constructor whose constraints are not
// satisfied is not eligible, so a trivial-but-ineligible defaulted
// default constructor does not make a class implicit-lifetime
// (`__builtin_is_implicit_lifetime`, clang 22 `isIneligibleOrNotSelected`).
// Oracle-checked with clang++-22: same answers, no diagnostics.
//@ requires: clang>=22
//@ strict-diags

template <bool B>
struct Constrained {
  Constrained()
    requires B
  = default;
  Constrained(const Constrained&) {}
};

static_assert(__builtin_is_implicit_lifetime(Constrained<true>));
static_assert(!__builtin_is_implicit_lifetime(Constrained<false>));

template <bool B>
struct CopyOnly {
  CopyOnly() {}
  CopyOnly(const CopyOnly&)
    requires B
  = default;
};

static_assert(__builtin_is_implicit_lifetime(CopyOnly<true>));
static_assert(!__builtin_is_implicit_lifetime(CopyOnly<false>));
