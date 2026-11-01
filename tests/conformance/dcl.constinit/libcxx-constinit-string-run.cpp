// [dcl.constinit], [basic.start.static]/2: a namespace-scope std::string
// initialized from a short literal is constant-initialized (libc++'s
// short-string union; its active member is set by assignments in the
// constructor), so it is emitted as a constant, as clang does.
// spec/oven.md "Transient allocation" (WP-212).
//@ args: -stdlib=libc++
//@ expect: run
//@ stdout: ABC 3 DEF
#include <cstdio>
#include <string>

constinit std::string s = "ABC";
std::string arr[] = {"DEF", "GHI"};

int main() { std::printf("%s %zu %s\n", s.c_str(), s.size(), arr[0].c_str()); }
