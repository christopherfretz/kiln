// spec/host-calls.md §2 (WP-1990): the inputs arrive exactly as many
// elements as their lengths say (no terminator, an embedded NUL kept),
// from a vector, a string, a literal and the middle of an array; exactly
// the elements the handler wrote are stored; an output overlapping an
// input is defined (the inputs are read first); the handler sees both
// capacities. The test handler's op 2 echoes, op 4 returns the
// capacities.
//@ args: -fkiln-host-call-test-handler
//@ skip(clang): kiln's host-call builtin
#include <string>
#include <vector>
// The wrapper the tests call through (spec/host-calls.md §1).
using u64 = unsigned long long;
constexpr u64 hc(unsigned op, const u64 *in = nullptr, unsigned n_in = 0,
                 const char *bin = nullptr, unsigned n_bin = 0,
                 u64 *out = nullptr, unsigned cap_out = 0,
                 char *bout = nullptr, unsigned cap_bout = 0) {
  return __builtin_kiln_host_call(op, in, n_in, bin, n_bin, out, cap_out,
                                  bout, cap_bout);
}

consteval bool containers() {
  std::vector<u64> v{1, 2, 0xffffffffffffffffull};
  std::string s = "hello";
  u64 o[3]{};
  char b[5]{};
  if (hc(2, v.data(), 3, s.data(), 5, o, 3, b, 5) != ((3ull << 32) | 5))
    return false;
  return o[0] == 1 && o[2] == 0xffffffffffffffffull && b[0] == 'h' &&
         b[4] == 'o';
}
static_assert(containers());

consteval bool embedded_nul() {
  const char lit[] = "a\0b";
  char b[3]{};
  if (hc(2, nullptr, 0, lit, 3, nullptr, 0, b, 3) != 3)
    return false;
  return b[0] == 'a' && b[1] == 0 && b[2] == 'b';
}
static_assert(embedded_nul());

consteval bool mid_array() {
  u64 a[5] = {10, 20, 30, 40, 50};
  u64 o[2]{};
  if (hc(2, a + 2, 2, nullptr, 0, o, 2) != (2ull << 32))
    return false;
  return o[0] == 30 && o[1] == 40;
}
static_assert(mid_array());

// Capacities of 0 with null outputs: only the value comes back.
consteval u64 nothing_out() { return hc(2, nullptr, 0, nullptr, 0); }
static_assert(nothing_out() == 0);

// Only the written elements change.
consteval bool partial() {
  u64 in[1] = {7};
  u64 o[3] = {1, 1, 1};
  hc(2, in, 1, nullptr, 0, o, 3);
  return o[0] == 7 && o[1] == 1 && o[2] == 1;
}
static_assert(partial());

consteval bool overlap() {
  u64 buf[4] = {1, 2, 3, 0};
  hc(2, buf, 3, nullptr, 0, buf + 1, 3);
  return buf[0] == 1 && buf[1] == 1 && buf[2] == 2 && buf[3] == 3;
}
static_assert(overlap());

consteval u64 caps() {
  u64 o[3];
  char b[5];
  return hc(4, nullptr, 0, nullptr, 0, o, 3, b, 5);
}
static_assert(caps() == ((3ull << 32) | 5));
