//@ expect: run
//@ std: c++20
//@ stdout: 4 4 -10
// A non-dependent template argument that an instantiation keeps from its
// pattern still odr-uses what it names ([basic.def.odr]/4,
// [temp.inst]/4): the member functions below are instantiated and
// defined, so the program links.
#include <cstdio>
#include <tuple>

struct S {
  template <auto C, typename T> void call(T &t) { (t.*C)(1); }
};
template <auto C, typename T> void call(T &t) { (t.*C)(1); }

template <int N> struct H {
  int n = 0;
  void a(int e) { n += e; }
  void b(int e) { n += e; }
  void c(int e) { n += e; }
  void d(int e) { n += e; }
  H() {
    call<&H::a>(*this);
    [this] { call<&H::b>(*this); }();
    [this](auto) { call<&H::c>(*this); }(0);
    [this](auto s) { s.template call<&H::d>(*this); }(S{});
  }
};

template <auto Candidate, typename Type> void (*wrap())(void *, int) {
  return [](void *p, int a) { (static_cast<Type *>(p)->*Candidate)(a); };
}
struct sink {
  void (*fn)(void *, int) = nullptr;
  void *inst = nullptr;
  template <auto C, typename T> void connect(T &t) {
    inst = &t;
    fn = wrap<C, T>();
  }
  void operator()(int a) { fn(inst, a); }
};
template <int N> struct handler {
  int n = 0;
  void remove_if(int e) { n -= N * e; }
  template <typename... P> handler(std::tuple<P &...> ps) {
    std::apply([this](auto &...s) { (s.template connect<&handler::remove_if>(*this), ...); }, ps);
  }
};

int main() {
  H<1> h1;
  H<2> h2;
  std::printf("%d %d ", h1.n, h2.n);
  sink a;
  handler<2> h{std::tuple<sink &>(a)};
  a(5);
  std::printf("%d\n", h.n);
}
