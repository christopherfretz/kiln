// [dcl.init.list]/3.7, [over.match.list]/1.1: `V a{{}}` with an
// initializer-list constructor selects it with a list of ONE element,
// value-initialized from the inner `{}`; only an empty outer list with a
// default constructor is value-initialization (`V a{}`). The class
// template std::initializer_list declared here has public data members,
// so it is an aggregate: a braced list initializing it is still the
// initializer_list's own initialization ([dcl.init.list]/5), not
// aggregate initialization of its members. Found running GCC's C++
// testsuite.
//@ expect: run
//@ strict-diags
//@ stdout: -1 1 5 2 10 1 3 1 2
//@ stdout: 0
//@ stdout: 3
//@ stdout: 2
//@ stdout: 0
typedef decltype(sizeof(char)) size_t;
namespace std {
template<class T> struct initializer_list {
  const T *b_;
  size_t n_;
  constexpr size_t size() const { return n_; }
  constexpr const T *begin() const { return b_; }
};
}
extern "C" int printf(const char *, ...);
struct A { int e = 5; };
struct V {
  int n, s;
  V(std::initializer_list<A> a) : n((int)a.size()), s(0) {
    for (size_t i = 0; i < a.size(); ++i) s += a.begin()[i].e;
  }
  V() : n(-1), s(0) {}
};
struct VV {
  int n;
  VV(std::initializer_list<V> l) : n((int)l.size()) {}
};
struct CI {
  int n;
  constexpr CI(std::initializer_list<int> l) : n((int)l.size()) {}
};
constexpr CI c1{{}};
static_assert(c1.n == 1, "");
void f(std::initializer_list<int> l) { printf("%d\n", (int)l.size()); }
int main() {
  V a{};
  V b{{}};
  V c{{}, {}};
  V d = {{}};
  V e{A{3}};
  VV g{{}, {}};
  printf("%d %d %d %d %d %d %d %d %d\n", a.n, b.n, b.s, c.n, c.s, d.n, e.s, V({{}}).n, g.n);
  f({});
  f({1, 2, 3});
  std::initializer_list<int> l = {4, 5};
  printf("%d\n", (int)l.size());
  std::initializer_list<int> z = {};
  printf("%d\n", (int)z.size());
}
