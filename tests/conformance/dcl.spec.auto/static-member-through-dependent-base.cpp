//@ expect: run
//@ std: c++20
//@ stdout: 3 5 9
// A static member function with a placeholder return type, named as
// `this->f` or `obj.f` in an instantiation, has its return type deduced
// before the member access takes its type ([dcl.spec.auto.general]/12):
// the call's type is `int`, not `auto`.
#include <cstdio>
#include <type_traits>

template <class T1> int twice(const T1 &a) { return a + a; }

template <class T> struct F {
  using type = T;
  static auto push(int p, int e) {
    if constexpr (std::is_void_v<type>) {
      return p + e;
    } else {
      type i{};
      return p + e + static_cast<int>(i) + 2;
    }
  }
};
template <class T> struct X : F<T> {
  int body() { return twice(this->push(1, 0)) + 1; }
  int obj(F<T> &f) { return f.push(2, 3); }
};

int main() {
  X<void> v;
  X<int> i;
  F<void> f;
  static_assert(std::is_same_v<decltype(f.push(1, 1)), int>);
  std::printf("%d %d %d\n", v.body(), v.obj(f), i.body() + 2);
}
