// spec/host-calls.md §1 (WP-1990): without a registered handler the
// host-call builtin does not exist: __has_builtin answers 0 and the name
// is an ordinary undeclared identifier, as it is for clang.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 12:10 error "use of undeclared identifier '__builtin_kiln_host_call'"

static_assert(!__has_builtin(__builtin_kiln_host_call));
static_assert(!__has_constexpr_builtin(__builtin_kiln_host_call));

consteval unsigned long long f() {
  return __builtin_kiln_host_call(1, nullptr, 0, nullptr, 0, nullptr, 0,
                                  nullptr, 0);
}
