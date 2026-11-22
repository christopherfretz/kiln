// spec/host-calls.md §2.4 (WP-1990): a handler's error ends the
// evaluation, uncatchably, with its message; so does writing more than a
// capacity. A cancellation reported deep in a call stack ends it at once:
// no destructor of the evaluation's objects runs, so none reaches the
// handler (op 1, the counter, is still at 0 afterwards). Reading an
// output element the handler did not write is the usual
// uninitialized read. Test handler: op 3 fails with "boom", op 5 arms
// the flag op 6 fails on, op 2 echoes.
//@ expect: compile-fail
//@ args: -fkiln-host-call-test-handler
//@ skip(clang): kiln's host-call builtin
//@ strict-diags
//@ diag: 78:15 error "constexpr variable 'a' must be initialized by a constant expression"
//@ diag: 67:10 note "host call failed: boom"
//@ diag: 73:12 note "in call to 'hc(3, nullptr, 0, nullptr, 0, nullptr, 0, nullptr, 0)'"
//@ diag: 78:19 note "in call to 'boom()'"
//@ diag: 78:19 error "call to consteval function 'boom' is not a constant expression"
//@ diag: 67:10 note "host call failed: boom"
//@ diag: 73:12 note "in call to 'hc(3, nullptr, 0, nullptr, 0, nullptr, 0, nullptr, 0)'"
//@ diag: 78:19 note "in call to 'boom()'"
//@ diag: 85:15 error "constexpr variable 'b' must be initialized by a constant expression"
//@ diag: 67:10 note "host call handler wrote 2 integers into a capacity of 1"
//@ diag: 83:10 note "in call to 'hc(2, &in[0], 2, nullptr, 0, &out[0], 1, nullptr, 0)'"
//@ diag: 85:19 note "in call to 'over()'"
//@ diag: 85:19 error "call to consteval function 'over' is not a constant expression"
//@ diag: 67:10 note "host call handler wrote 2 integers into a capacity of 1"
//@ diag: 83:10 note "in call to 'hc(2, &in[0], 2, nullptr, 0, &out[0], 1, nullptr, 0)'"
//@ diag: 85:19 note "in call to 'over()'"
//@ diag: 93:15 error "constexpr variable 'c' must be initialized by a constant expression"
//@ diag: 91:10 note "read of uninitialized object is not allowed in a constant expression"
//@ diag: 93:19 note "in call to 'unwritten()'"
//@ diag: 93:19 error "call to consteval function 'unwritten' is not a constant expression"
//@ diag: 91:10 note "read of uninitialized object is not allowed in a constant expression"
//@ diag: 93:19 note "in call to 'unwritten()'"
//@ diag: 106:15 error "constexpr variable 'd' must be initialized by a constant expression"
//@ diag: 67:10 note "host call failed: cancelled"
//@ diag: 102:12 note "in call to 'hc(6, nullptr, 0, nullptr, 0, nullptr, 0, nullptr, 0)'"
//@ diag: 104:10 note "in call to 'deep(0)'"
//@ diag: 104:10 note "in call to 'deep(1)'"
//@ diag: 104:10 note "in call to 'deep(2)'"
//@ diag: 104:10 note "in call to 'deep(3)'"
//@ diag: 104:10 note "(skipping 92 calls in backtrace; use -fconstexpr-backtrace-limit=0 to see all)"
//@ diag: 104:10 note "in call to 'deep(96)'"
//@ diag: 104:10 note "in call to 'deep(97)'"
//@ diag: 104:10 note "in call to 'deep(98)'"
//@ diag: 104:10 note "in call to 'deep(99)'"
//@ diag: 106:19 note "in call to 'deep(100)'"
//@ diag: 106:19 error "call to consteval function 'deep' is not a constant expression"
//@ diag: 67:10 note "host call failed: cancelled"
//@ diag: 102:12 note "in call to 'hc(6, nullptr, 0, nullptr, 0, nullptr, 0, nullptr, 0)'"
//@ diag: 104:10 note "in call to 'deep(0)'"
//@ diag: 104:10 note "in call to 'deep(1)'"
//@ diag: 104:10 note "in call to 'deep(2)'"
//@ diag: 104:10 note "in call to 'deep(3)'"
//@ diag: 104:10 note "(skipping 92 calls in backtrace; use -fconstexpr-backtrace-limit=0 to see all)"
//@ diag: 104:10 note "in call to 'deep(96)'"
//@ diag: 104:10 note "in call to 'deep(97)'"
//@ diag: 104:10 note "in call to 'deep(98)'"
//@ diag: 104:10 note "in call to 'deep(99)'"
//@ diag: 106:19 note "in call to 'deep(100)'"
// The wrapper the tests call through (spec/host-calls.md §1).
using u64 = unsigned long long;
constexpr u64 hc(unsigned op, const u64 *in = nullptr, unsigned n_in = 0,
                 const char *bin = nullptr, unsigned n_bin = 0,
                 u64 *out = nullptr, unsigned cap_out = 0,
                 char *bout = nullptr, unsigned cap_bout = 0) {
  return __builtin_kiln_host_call(op, in, n_in, bin, n_bin, out, cap_out,
                                  bout, cap_bout);
}

consteval u64 boom() {
  try {
    return hc(3);
  } catch (...) {
    return 1;
  }
}
constexpr u64 a = boom();

consteval u64 over() {
  u64 in[2] = {7, 8};
  u64 out[1];
  return hc(2, in, 2, nullptr, 0, out, 1);
}
constexpr u64 b = over();

consteval u64 unwritten() {
  u64 in[1] = {7};
  u64 out[2];
  hc(2, in, 1, nullptr, 0, out, 2);
  return out[1];
}
constexpr u64 c = unwritten();

struct Counted {
  constexpr ~Counted() { hc(1); }
};
consteval u64 deep(int n) {
  Counted guard;
  if (n == 0) {
    hc(5);
    return hc(6);
  }
  return deep(n - 1);
}
constexpr u64 d = deep(100);
static_assert(hc(1) == 0);
