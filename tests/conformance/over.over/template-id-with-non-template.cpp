//@ expect: run
//@ std: c++20
//@ stdout: 1 2 3 4 5
// A template-id names only function templates ([temp.names]/3,
// [temp.arg.explicit]/4): a non-template function found by the same
// lookup is not a candidate, so `&rm<M>` names the one specialization
// even with no target type to resolve against ([over.over]/3).
#include <cstdio>

struct M {
  int v = 0;
};
int rm(const char *) { return -1; }
template <typename T> void rm(T &m, int v) { m.v += v; }

template <auto F> void call(M &m, int v) { F(m, v); }
template <void (*F)(M &, int)> void call_typed(M &m, int v) { F(m, v); }

int main() {
  M m;
  auto p = &rm<M>;
  p(m, 1);
  std::printf("%d ", m.v);
  auto q = rm<M>;
  q(m, 1);
  std::printf("%d ", m.v);
  call<&rm<M>>(m, 1);
  std::printf("%d ", m.v);
  call_typed<rm<M>>(m, 1);
  std::printf("%d ", m.v);
  decltype(&rm<M>) r = rm<M>;
  r(m, 1);
  std::printf("%d\n", m.v + rm("") + 1);
}
