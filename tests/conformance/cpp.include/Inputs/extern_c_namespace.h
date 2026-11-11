// C-linkage functions and variables declared in a namespace: each is the
// entity of its name at global scope (the shape of libc++ <cxxabi.h>).
#pragma once
extern "C" int ecn_first(int);
namespace ecn {
extern "C" {
int ecn_first(int);    // redeclares ::ecn_first
int ecn_second(int x); // first declared here
extern int ecn_var;
struct T {
  int v;
};
}
inline int use(int x) { return ecn_first(x) + ecn_second(x) + ecn_var; }
} // namespace ecn
extern "C" int ecn_second(int); // redeclares ecn::ecn_second
