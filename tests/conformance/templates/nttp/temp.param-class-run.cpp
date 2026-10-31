// [temp.param]/8: template parameter objects are emitted as constant globals
// (one per value) and have a stable address at run time.
//@ expect: run
//@ stdout: 3 3 1 6
extern "C" int printf(const char*, ...);
struct P { int x, y; };
template <P p> int sum() { return p.x + p.y; }
template <P p> const P* addr() { return &p; }
template <P p> struct S { int get() const { return p.y; } };
int main() {
  printf("%d %d %d %d\n", sum<P{1, 2}>(), addr<P{3, 4}>()->x, addr<P{3, 4}>() == addr<P{3, 4}>(), S<P{5, 6}>{}.get());
}
