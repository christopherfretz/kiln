// [stmt.ranged]/1.3: without members `begin` and `end` the range uses
// `begin(__range)` and `end(__range)` found by argument-dependent lookup
// only ([basic.lookup.argdep]); an ordinary-lookup `begin` is ignored.
//@ expect: run
//@ stdout: 9 7
extern "C" int printf(const char*, ...);

namespace lib {
struct Bag { int v[3] = {2, 3, 4}; };
const int* begin(const Bag& b) { return b.v; }
const int* end(const Bag& b) { return b.v + 3; }

template <class T> struct Box { T v[2]; };
template <class T> T* begin(Box<T>& b) { return b.v; }
template <class T> T* end(Box<T>& b) { return b.v + 2; }
}

int* begin(lib::Bag&);  // not found: only ADL is used

int main() {
  lib::Bag b;
  int s = 0;
  for (int x : b) s += x;
  lib::Box<int> bx{{3, 4}};
  int t = 0;
  for (int& x : bx) t += x;
  printf("%d %d\n", s, t);
}
