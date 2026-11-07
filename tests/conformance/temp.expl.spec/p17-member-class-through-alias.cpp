//@ expect: run
//@ stdout: 2 3 1 4
// WP-782: [temp.expl.spec]/1, [temp.expl.spec]/17: a member class of an
// implicitly instantiated class template specialization is explicitly
// specialized with the specialization named by a typedef or alias
// declaration as well as by its template-id (LLVM's CGSCCPassManager.h:
// `template <> class CGSCCAnalysisManagerModuleProxy::Result`). Its
// members are then defined without `template<>` ([temp.expl.spec]/5).
#include <cstdio>

template <class A, class B> struct Proxy {
  struct Result {
    int x() { return 1; }
  };
};
struct M {};
struct N {};
using P = Proxy<M, N>;
typedef Proxy<N, M> Q;

template <> class P::Result {
public:
  int x() { return 2; }
};
template <> struct Proxy<N, N>::Result {
  int x() { return 3; }
};
template <> struct Q::Result;
template <> struct Q::Result {
  int x();
};
int Q::Result::x() { return 4; }

int main() {
  std::printf("%d %d %d %d\n", P::Result().x(), Proxy<N, N>::Result().x(),
              Proxy<M, M>::Result().x(), Q::Result().x());
}
