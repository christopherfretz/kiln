// Function-name spans (spec/ast.md §24.4): an operator function's name is
// one entry over the whole `operator...` spelling.
//@ fn-names: names-operators.names
//@ std: c++20
#include <stddef.h>
namespace ns {
struct S {
  bool operator==(const S &) const { (void)__PRETTY_FUNCTION__; return true; }
  int operator()(int) { (void)__PRETTY_FUNCTION__; return 0; }
  int operator[](int) { (void)__func__; return 0; }
  static void *operator new(size_t n) { (void)__PRETTY_FUNCTION__; return ::operator new(n); }
  S &operator<<=(int) { (void)__FUNCTION__; return *this; }
};
int operator""_x(unsigned long long) { (void)__PRETTY_FUNCTION__; return 0; }
}
using namespace ns;
void use() { S s; (void)(s == s); s(1); s[1]; delete new S; s <<= 1; (void)1_x; }
