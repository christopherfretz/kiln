// [temp.arg.nontype]/1, [expr.const]/22: a constant template argument is
// a constant-expression, manifestly constant-evaluated, also when it is
// written in a template and substituted at instantiation: an immediate
// invocation in it is part of that one constant expression, so the
// std::vector a consteval call returns need not be a constant result of
// its own. kiln substituted explicit template arguments of a function
// call outside a constant-evaluated context ("invalid explicitly-specified
// argument"). A function, static and non-static member template, a
// generic lambda's operator(), a class and a variable template, a member
// template of a class template. WP-1613.
//@ expect: run
//@ strict-diags
#include <vector>
consteval std::vector<int> vec(int n) { return std::vector<int>(n, 7); }
template <int K> int fi() { return K; }
struct M { template <int K> static int sm() { return K; } template <int K> int m() const { return K; } };
template <int K> struct C { static constexpr int v = K; };
template <int K> constexpr int vt = K;
template <int N> int f() {
  int s = fi<vec(N).size()>();
  s += M::sm<vec(N).size()>();
  s += M{}.template m<vec(N).size()>();
  s += []<int K>() { return K; }.template operator()<vec(N).size()>();
  s += fi<vec(N)[N - 1]>();
  s += C<vec(N).size()>::v + vt<vec(N).size()>;
  return s;
}
template <class T> struct X {
  template <int N> int g() { return fi<vec(N + sizeof(T)).size()>(); }
};
int main() { return f<2>() == 2 + 2 + 2 + 2 + 7 + 4 && X<char>().g<1>() == 2 ? 0 : 1; }
