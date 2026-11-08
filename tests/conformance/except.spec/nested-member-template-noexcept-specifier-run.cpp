// [except.spec]/13, [temp.inst]/14: the exception specification of a
// member function template specialization is instantiated when it is
// needed, and `noexcept(e)` is true when no potentially-throwing function
// is called. Here `f`'s noexcept-specifier calls the member template `g`,
// and `f`'s is first needed while `call`'s noexcept-specifier is being
// instantiated. Boost.HOF `fold` and `reverse_fold` (`v_fold`) hit this.
//@ expect: run
//@ stdout: 1 1
extern "C" int printf(const char*, ...);
struct V {
  template<class T>
  constexpr int f(int s, T x) const noexcept(noexcept(g(s + x))) { return g(s + x); }
  template<class T>
  constexpr int g(T s) const noexcept { return s; }
};
template<class T>
constexpr int call(T x) noexcept(noexcept(V().f(0, x))) { return V().f(0, x); }

int main() {
  bool a = noexcept(call(2));
  bool b = noexcept(V().f(0, 2));
  printf("%d %d\n", (int)a, (int)b);
}
