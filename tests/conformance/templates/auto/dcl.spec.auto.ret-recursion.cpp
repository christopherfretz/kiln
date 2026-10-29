// [dcl.spec.auto.general]/11: once a return statement has been seen, the
// return type is deduced and the function may recurse.
//@ strict-diags
constexpr auto fact(int n) {
  if (n <= 1)
    return 1;
  return n * fact(n - 1);
}
static_assert(fact(5) == 120);
static_assert(__is_same(decltype(fact(1)), int));
