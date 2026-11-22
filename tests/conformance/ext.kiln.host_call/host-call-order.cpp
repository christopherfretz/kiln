// spec/host-calls.md §2, §5 (WP-1990): each call reaches the handler
// once, in evaluation order, and no frame around one is memoized: the
// test handler's op 1 is a counter, op 0 counts the calls before it.
//@ args: -fkiln-host-call-test-handler
//@ skip(clang): kiln's host-call builtin
// The wrapper the tests call through (spec/host-calls.md §1).
using u64 = unsigned long long;
constexpr u64 hc(unsigned op, const u64 *in = nullptr, unsigned n_in = 0,
                 const char *bin = nullptr, unsigned n_bin = 0,
                 u64 *out = nullptr, unsigned cap_out = 0,
                 char *bout = nullptr, unsigned cap_bout = 0) {
  return __builtin_kiln_host_call(op, in, n_in, bin, n_bin, out, cap_out,
                                  bout, cap_bout);
}

static_assert(__has_builtin(__builtin_kiln_host_call));
static_assert(__has_constexpr_builtin(__builtin_kiln_host_call));

consteval u64 three() {
  u64 a = hc(1);
  u64 b = hc(1);
  u64 c = hc(1);
  return a * 100 + b * 10 + c;
}
static_assert(three() == 12);

// The same function with the same arguments, directly and from two
// callers: a new value each time.
constexpr u64 next() { return hc(1); }
consteval u64 one() { return next(); }
consteval u64 two() { return next(); }
static_assert(next() == 3);
static_assert(next() == 4);
static_assert(one() == 5);
static_assert(two() == 6);
static_assert(one() == 7);
consteval u64 twice() { return next() * 10 + next(); }
static_assert(twice() == 89);

// A definition calls nothing; its evaluation calls once per evaluated
// call (a const local used as a value is read from its frame).
consteval u64 local() {
  const u64 n = hc(1);
  return n + n;
}
static_assert(hc(0) == 10);
static_assert(local() == 20);
static_assert(hc(0) == 12);

// Unevaluated operands call nothing.
static_assert(!noexcept(hc(1)));
static_assert(sizeof(hc(1)) == 8);
decltype(hc(1)) unevaluated = 0;
template <class T> concept Callable = requires { hc(1); };
static_assert(Callable<int>);
static_assert(hc(0) == 13);

// The results of a constexpr variable, a template argument and a case
// label: each one evaluation.
constexpr u64 v = hc(1);
static_assert(v == 11);
template <u64 N> constexpr u64 arg = N;
static_assert(arg<hc(1)> == 12);
consteval int label(u64 x) {
  switch (x) {
  case hc(1): return 1;
  default: return 0;
  }
}
static_assert(label(13) == 1);
