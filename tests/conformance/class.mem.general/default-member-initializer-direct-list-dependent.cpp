// [class.mem.general]/11, [dcl.init.general]/15, [temp.inst]: a default
// member initializer `T m{x}` is direct-list-initialization, so an
// explicit constructor is usable. In a class template with `x` only
// value-dependent the pattern builds the construction already; its
// instantiation must stay direct-list-initialization rather than become
// copy-initialization from the built object (Boost.Hana's laws/base.hpp
// `Tracked tracker{i}`).
//@ expect: run
//@ stdout: 1 2 3 4
extern "C" int printf(const char*, ...);

struct Tracked {
  explicit Tracked(int k) : value{k} {}
  int value;
};
template <int i> struct integer {
  Tracked tracker{i};
};
template <int i> struct eq : integer<i> {};
template <class T> struct typed {
  Tracked tracker{T(3)};
};
template <int i> struct twice {
  Tracked tracker{i * 2};
};

int main() {
  eq<1> a{};
  auto b = eq<2>{};
  typed<int> c;
  twice<2> d;
  printf("%d %d %d %d\n", a.tracker.value, b.tracker.value, c.tracker.value,
         d.tracker.value);
}
