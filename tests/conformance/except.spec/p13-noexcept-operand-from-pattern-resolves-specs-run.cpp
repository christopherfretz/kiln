// [except.spec]/13, [expr.unary.noexcept]: `noexcept(V().g(x))` in the
// noexcept-specifier of a function template needs the exception
// specification of `V`'s implicit default constructor when `call<int>`'s
// is instantiated; it is non-throwing ([except.spec]/8), so `call(2)` is
// noexcept even when nothing resolved it before.
//@ expect: run
//@ stdout: 1
extern "C" int printf(const char*, ...);
struct V {
  constexpr int g(int s) const noexcept { return s; }
};
template<class T>
constexpr int call(T x) noexcept(noexcept(V().g(x))) { return V().g(x); }

int main() {
  bool a = noexcept(call(2));
  printf("%d\n", (int)a);
}
