//@ expect: run
//@ std: c++20
//@ stdout: 1 1 7 1 7 9 4
// [expr.prim.id.general]/3, [expr.unary.op]/3: `&C::m` in a lambda names
// the member, not `this->C::m`, so it captures nothing
// ([expr.prim.lambda.capture]/8), in a class and in a class template.
#include <cstdio>

template <auto P, typename T> int call(T &t) { return (t.*P)(); }

struct H {
  int x = 4;
  int g() { return 7; }
  int run() {
    auto l = [] { return &H::g; };
    auto m = [=] { return &H::x; };
    int (*fp)(H &) = [](H &o) { return call<&H::g>(o); };
    std::printf("%d %d %d ", static_cast<int>(sizeof(l)), static_cast<int>(sizeof(m)), fp(*this));
    return (this->*l())() * 0 + this->*m();
  }
};

template <int N> struct T {
  int g() { return N; }
  int run() {
    auto l = [] { return &T::g; };
    auto k = [](auto self) { return (self->*(&T::g))(); };
    std::printf("%d %d ", static_cast<int>(sizeof(l)), (this->*l())() - 2);
    return k(this);
  }
};

int main() {
  H h;
  int x = h.run();
  T<9> t;
  int y = t.run();
  std::printf("%d %d\n", y, x);
}
