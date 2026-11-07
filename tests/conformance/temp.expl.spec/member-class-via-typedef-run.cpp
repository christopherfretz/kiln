// [temp.expl.spec]/1, /16: a member class of an implicitly instantiated
// class template specialization can be explicitly specialized, and the
// specialization may name the enclosing specialization through a typedef
// (`template<> class P::Result {}` with `P` an alias of `Proxy<int>`), not
// only as the template-id `Proxy<int>::Result`. [temp.expl.spec]/5:
// members of the explicitly specialized class are defined without
// `template<>`. LLVM's `LoopAnalysisManagerFunctionProxy::Result`
// (LoopAnalysisManager.h). Verified against clang 19.
//@ expect: run
//@ strict-diags
//@ std: c++17
//@ stdout: 7 8 1 42 1 2
#include <cstdio>

template <class T> struct Proxy {
  class Result {
  public:
    int x = 1;
  };
  Result run(T&);
};
template <class T> typename Proxy<T>::Result Proxy<T>::run(T&) { return Result(); }

using P = Proxy<int>;
typedef Proxy<short> S;

template <> class P::Result {
public:
  explicit Result(int v) : x(v) {}
  bool invalidate(int k) const;
  int x;
};
template <> P::Result P::run(int& v);

template <> class S::Result final {
public:
  int x = 2;
};

template <> class Proxy<long>::Result {
public:
  int x = 8;
};

bool P::Result::invalidate(int k) const { return k == x; }
template <> P::Result P::run(int& v) { return Result(v * 6); }

int main() {
  int seven = 7;
  P p;
  Proxy<char> pc;
  char c = 0;
  short s = 0;
  std::printf("%d %d %d %d %d %d\n", P::Result(seven).x, Proxy<long>::Result().x,
              pc.run(c).x, p.run(seven).x, p.run(seven).invalidate(42),
              S().run(s).x);
}
