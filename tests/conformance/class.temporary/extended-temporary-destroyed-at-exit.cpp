// [class.temporary]/6, [basic.start.term]/1 (WP-231): a temporary of class
// type with a non-trivial destructor bound to a namespace-scope reference
// is dynamically initialized and destroyed after main returns.
//@ expect: run
//@ stdout: ctor 1 main 1 3 dtor 3 dtor 1 end
#include <cstdio>
struct Last { ~Last() { std::printf(" end\n"); } } last;
struct D {
  int a;
  constexpr D(int x) : a(x) {}
  ~D() { std::printf(" dtor %d", a); }
};
const D& r = D{1};
D&& r2 = D{3};
struct Probe { Probe() { std::printf("ctor %d", r.a); } } probe;
int main() { std::printf(" main %d %d", r.a, r2.a); }
