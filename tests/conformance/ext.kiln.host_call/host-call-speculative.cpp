// spec/host-calls.md §4 (WP-1990): an evaluation that asks whether
// something is a constant expression, to decide what the program means,
// may not call the handler: the error "host call during speculative
// evaluation", and the handler is not called (op 0 counts the calls
// before it). Here: the hidden second evaluation of a local's
// initializer for an array bound, and whether a namespace-scope const
// variable is constant-initialized, and the argument of
// __builtin_constant_p.
//@ expect: compile-fail
//@ args: -fkiln-host-call-test-handler -Wno-vla-cxx-extension
//@ skip(clang): kiln's host-call builtin
//@ strict-diags
//@ diag: 34:17 error "host call during speculative evaluation"
//@ diag: 31:3 note "expanded from macro 'CALL1'"
//@ diag: 40:15 error "host call during speculative evaluation"
//@ diag: 31:3 note "expanded from macro 'CALL1'"
//@ diag: 42:37 error "host call during speculative evaluation"
//@ diag: 31:3 note "expanded from macro 'CALL1'"
// The wrapper the tests call through (spec/host-calls.md §1).
using u64 = unsigned long long;
constexpr u64 hc(unsigned op, const u64 *in = nullptr, unsigned n_in = 0,
                 const char *bin = nullptr, unsigned n_bin = 0,
                 u64 *out = nullptr, unsigned cap_out = 0,
                 char *bout = nullptr, unsigned cap_bout = 0) {
  return __builtin_kiln_host_call(op, in, n_in, bin, n_bin, out, cap_out,
                                  bout, cap_bout);
}

// Called directly, so the error is located here.
#define CALL1()                                                                \
  __builtin_kiln_host_call(1, nullptr, 0, nullptr, 0, nullptr, 0, nullptr, 0)

consteval u64 bound() {
  const u64 n = CALL1() + 1;
  int a[n];
  a[0] = 1;
  return 0;
}

const u64 g = CALL1();

static_assert(!__builtin_constant_p(CALL1()));

static_assert(hc(0) == 0);
