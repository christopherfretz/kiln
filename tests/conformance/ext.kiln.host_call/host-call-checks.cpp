// spec/host-calls.md §2.2 (WP-1990): every range is checked before the
// handler runs; a failed check makes the call not a constant expression,
// with a note naming the argument, and the handler is not called (the
// test handler's op 0 counts the calls before it).
//@ expect: compile-fail
//@ args: -fkiln-host-call-test-handler
//@ skip(clang): kiln's host-call builtin
//@ strict-diags
//@ diag: 71:15 error "constexpr variable 'a' must be initialized by a constant expression"
//@ diag: 63:10 note "'in' of __builtin_kiln_host_call does not point to 2 consecutive elements"
//@ diag: 69:10 note "in call to 'hc(2, &in[0], 2, nullptr, 0, nullptr, 0, nullptr, 0)'"
//@ diag: 71:19 note "in call to 'short_in()'"
//@ diag: 71:19 error "call to consteval function 'short_in' is not a constant expression"
//@ diag: 63:10 note "'in' of __builtin_kiln_host_call does not point to 2 consecutive elements"
//@ diag: 69:10 note "in call to 'hc(2, &in[0], 2, nullptr, 0, nullptr, 0, nullptr, 0)'"
//@ diag: 71:19 note "in call to 'short_in()'"
//@ diag: 74:15 error "constexpr variable 'b' must be initialized by a constant expression"
//@ diag: 63:10 note "'in' of __builtin_kiln_host_call is null with a length of 1"
//@ diag: 73:34 note "in call to 'hc(2, nullptr, 1, nullptr, 0, nullptr, 0, nullptr, 0)'"
//@ diag: 74:19 note "in call to 'null_in()'"
//@ diag: 74:19 error "call to consteval function 'null_in' is not a constant expression"
//@ diag: 63:10 note "'in' of __builtin_kiln_host_call is null with a length of 1"
//@ diag: 73:34 note "in call to 'hc(2, nullptr, 1, nullptr, 0, nullptr, 0, nullptr, 0)'"
//@ diag: 74:19 note "in call to 'null_in()'"
//@ diag: 81:15 error "constexpr variable 'c' must be initialized by a constant expression"
//@ diag: 63:10 note "'in' of __builtin_kiln_host_call element 1 is uninitialized"
//@ diag: 79:10 note "in call to 'hc(2, &in[0], 2, nullptr, 0, nullptr, 0, nullptr, 0)'"
//@ diag: 81:19 note "in call to 'uninit_in()'"
//@ diag: 81:19 error "call to consteval function 'uninit_in' is not a constant expression"
//@ diag: 63:10 note "'in' of __builtin_kiln_host_call element 1 is uninitialized"
//@ diag: 79:10 note "in call to 'hc(2, &in[0], 2, nullptr, 0, nullptr, 0, nullptr, 0)'"
//@ diag: 81:19 note "in call to 'uninit_in()'"
//@ diag: 87:15 error "constexpr variable 'd' must be initialized by a constant expression"
//@ diag: 63:10 note "'out' of __builtin_kiln_host_call designates a const object"
//@ diag: 85:10 note "in call to 'hc(4, nullptr, 0, nullptr, 0, &k[0], 1, nullptr, 0)'"
//@ diag: 87:19 note "in call to 'const_out()'"
//@ diag: 87:19 error "call to consteval function 'const_out' is not a constant expression"
//@ diag: 63:10 note "'out' of __builtin_kiln_host_call designates a const object"
//@ diag: 85:10 note "in call to 'hc(4, nullptr, 0, nullptr, 0, &k[0], 1, nullptr, 0)'"
//@ diag: 87:19 note "in call to 'const_out()'"
//@ diag: 92:15 error "constexpr variable 'e' must be initialized by a constant expression"
//@ diag: 63:10 note "'bout' of __builtin_kiln_host_call designates a string literal"
//@ diag: 90:10 note "in call to 'hc(4, nullptr, 0, nullptr, 0, nullptr, 0, &"
//@ diag: 92:19 note "in call to 'literal_out()'"
//@ diag: 92:19 error "call to consteval function 'literal_out' is not a constant expression"
//@ diag: 63:10 note "'bout' of __builtin_kiln_host_call designates a string literal"
//@ diag: 90:10 note "in call to 'hc(4, nullptr, 0, nullptr, 0, nullptr, 0, &"
//@ diag: 92:19 note "in call to 'literal_out()'"
//@ diag: 96:15 error "constexpr variable 'f' must be initialized by a constant expression"
//@ diag: 63:10 note "'out' of __builtin_kiln_host_call designates an object whose lifetime began outside the evaluation"
//@ diag: 95:38 note "in call to 'hc(4, nullptr, 0, nullptr, 0, &outside[0], 2, nullptr, 0)'"
//@ diag: 96:19 note "in call to 'outside_out()'"
//@ diag: 96:19 error "call to consteval function 'outside_out' is not a constant expression"
//@ diag: 63:10 note "'out' of __builtin_kiln_host_call designates an object whose lifetime began outside the evaluation"
//@ diag: 95:38 note "in call to 'hc(4, nullptr, 0, nullptr, 0, &outside[0], 2, nullptr, 0)'"
//@ diag: 96:19 note "in call to 'outside_out()'"
// The wrapper the tests call through (spec/host-calls.md §1).
using u64 = unsigned long long;
constexpr u64 hc(unsigned op, const u64 *in = nullptr, unsigned n_in = 0,
                 const char *bin = nullptr, unsigned n_bin = 0,
                 u64 *out = nullptr, unsigned cap_out = 0,
                 char *bout = nullptr, unsigned cap_bout = 0) {
  return __builtin_kiln_host_call(op, in, n_in, bin, n_bin, out, cap_out,
                                  bout, cap_bout);
}

consteval u64 short_in() {
  u64 in[1] = {1};
  return hc(2, in, 2);
}
constexpr u64 a = short_in();

consteval u64 null_in() { return hc(2, nullptr, 1); }
constexpr u64 b = null_in();

consteval u64 uninit_in() {
  u64 in[2];
  in[0] = 1;
  return hc(2, in, 2);
}
constexpr u64 c = uninit_in();

consteval u64 const_out() {
  const u64 k[1] = {0};
  return hc(4, nullptr, 0, nullptr, 0, const_cast<u64 *>(k), 1);
}
constexpr u64 d = const_out();

consteval u64 literal_out() {
  return hc(4, nullptr, 0, nullptr, 0, nullptr, 0, const_cast<char *>("ab"), 2);
}
constexpr u64 e = literal_out();

u64 outside[2];
consteval u64 outside_out() { return hc(4, nullptr, 0, nullptr, 0, outside, 2); }
constexpr u64 f = outside_out();

static_assert(hc(0) == 0);
