//@ expect: run
//@ stdout: ctor 1 2|tmpl -1|conv 7|tmpl-conv 0|assign 9|tmpl-assign 0
// [temp.expl.spec]/1, [temp.deduct.decl]: `template<>` before an
// out-of-line constructor, conversion function or `operator=`
// declarator that matches no non-template member names an explicit
// specialization of the member template, deduced from the declared
// type; uses of that specialization take the specialized body.
#include <cstdio>

struct J { int v; };
struct K { int v; };

struct P final {
  P(int x_ = 0, int y_ = 0) : x(x_), y(y_) {}
  template <class T> P(const T&) : x(-1), y(-1) {}
  template <class T> operator T() const { return T{}; }
  template <class T> P& operator=(const T&) { x = 0; return *this; }
  int x = 0, y = 0;
};

template <> inline P::P(const J& j) : P(j.v, j.v + 1) {}
template <> P::operator J() const { return J{x + 6}; }
template <> P& P::operator=(const J& j) { x = j.v; return *this; }

int main() {
  J j{1};
  P p = j;
  std::printf("ctor %d %d|", p.x, p.y);
  P q = K{5};
  std::printf("tmpl %d|", q.x);
  J c = p;
  std::printf("conv %d|", c.v);
  K k = p;
  std::printf("tmpl-conv %d|", k.v);
  p = J{9};
  std::printf("assign %d|", p.x);
  q = K{3};
  std::printf("tmpl-assign %d\n", q.x);
}
