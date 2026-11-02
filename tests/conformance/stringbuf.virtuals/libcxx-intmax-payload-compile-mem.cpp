// [stringbuf.virtuals]/8, [string.cons]/12, [ostream.unformatted]/5: the
// shape of libc++'s stringstream `gcount.pass.cpp`/`tellp` large tests,
// a `std::string` of INT_MAX - 1 characters written into a
// `std::stringstream`. The payload is a run-time allocation of the test
// binary (about 2 GiB, and the stringbuf grows past 4 GiB holding a
// copy); nothing here is a constant expression, so the compile must stay
// a few tens of MiB however large the count is. The 5 GiB peak the
// libc++ harness reported for `gcount.pass.cpp` is that binary's, not
// kiln's (spec/libcxx.md "Peak RSS"). Only compiled: running it needs
// more memory than the corpus budget.
//@ expect: compile-ok
//@ args: -stdlib=libc++
//@ mem<=1024
#include <cassert>
#include <climits>
#include <sstream>
#include <string>

int main() {
  std::stringstream ss;
  std::string payload(INT_MAX - 1, '\0');
  ss.write(payload.data(), payload.size());
  assert(ss.tellp() == INT_MAX - 1);
  ss.write("a", 1);
  assert(ss.tellp() == INT_MAX);
  return 0;
}
