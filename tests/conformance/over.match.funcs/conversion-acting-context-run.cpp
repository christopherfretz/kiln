// [over.match.funcs.general]/5, [over.match.conv]/1, [conv.general]/4: a
// conversion function inherited from a base is a candidate whose implicit
// object parameter is the class of the object, so its implicit object
// argument needs no derived-to-base conversion and is ranked like the
// derived class's own: in a contextual conversion to bool a public
// `operator bool` of a base (exact match) beats a private `operator
// void*` of the derived class (a boolean conversion), and access is
// checked on the selected function only. The object is converted to the
// base when the call is built (a base at a non-zero offset). Found
// running GCC's C++ testsuite.
//@ expect: run
//@ strict-diags
//@ stdout: 1 1 0 1 1 3 1
//@ stdout: 1 0 1
#include <cstdio>
struct Pad { int pad[4] = {1, 2, 3, 4}; };
struct X { int v = 7; operator bool() const { return v == 7; } };
struct Y : Pad, X { private: operator void*() const { return nullptr; } };
struct XE { explicit operator bool() const { return true; } };
struct YE : XE { private: operator void*() const { return nullptr; } };
struct CX { constexpr operator bool() const { return true; } };
struct CY : CX { private: constexpr operator void*() const { return nullptr; } };
static_assert(CY{}, "");
void nf() noexcept(CY{});
static_assert(noexcept(nf()), "");
template<class T> struct E { explicit(T{}) E(int) {} };
static_assert(!__is_convertible(int, E<CY>), "");
int main() {
  Y y;
  const Y &r = y;
  bool b(r);
  bool c = r;
  int n = 0;
  while (r) { ++n; break; }
  for (; r;) { ++n; break; }
  if (r && y) ++n;
  std::printf("%d %d %d %d %d %d %d\n", bool(r), r ? 1 : 0, !r, b, c, n, bool(r) || !r);
  YE e;
  std::printf("%d %d %d\n", bool(e), !e, e ? 1 : 0);
  (void)E<CY>(1);
  return 0;
}
