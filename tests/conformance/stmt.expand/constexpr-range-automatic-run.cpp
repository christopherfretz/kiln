//@ expect: run
//@ skip(clang): neither clang 22 nor GCC 16 implements P2686R5 constexpr references to automatic objects (and clang 22 lacks expansion statements)
//@ stdout: 6 1 9 1 13 1
// [stmt.expand]/5.2-5.3, [expr.const.init]/2 (C++26, P2686R5): a constexpr
// expansion over a local glvalue range binds it by reference, so at run time
// each element designates the local's own subobject in this invocation
// (the address comparisons are not folded to a copy's).
extern "C" int printf(const char *, ...);
struct NC {
  int v[3];
  constexpr NC(int a, int b, int c) : v{a, b, c} {}
  NC(const NC &) = delete;
  constexpr const int *begin() const { return v; }
  constexpr const int *end() const { return v + 3; }
};
struct P { int a, b; };
int main() {
  constexpr NC nc(1, 2, 3);
  int s = 0, same = 1;
  template for (constexpr const int &x : nc) {
    s += x;
    const int *volatile q = &x;
    same &= q >= nc.v && q < nc.v + 3;
  }
  constexpr P p{4, 5};
  int s2 = 0, same2 = 1;
  template for (constexpr const int &x : p) {
    s2 += x;
    const int *volatile q = &x;
    same2 &= q == &p.a || q == &p.b;
  }
  constexpr int arr[2] = {6, 7};
  int s3 = 0, same3 = 1;
  template for (constexpr const int &x : arr) {
    s3 += x;
    const int *volatile q = &x;
    same3 &= q == arr || q == arr + 1;
  }
  printf("%d %d %d %d %d %d\n", s, same, s2, same2, s3, same3);
}
