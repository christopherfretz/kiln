// [namespace.udir]/2, [temp.inst]: a using-directive at block scope in a
// function template is replayed when the template is instantiated
//@ expect: run
//@ strict-diags
namespace lit { constexpr int operator""_k(unsigned long long v) { return int(v) * 1000; } int base = 4; }
template <class T> T scaled(T x) {
  using namespace lit;
  return x + 2_k + base;
}
int main() { return scaled(1) == 2005 ? 0 : 1; }
