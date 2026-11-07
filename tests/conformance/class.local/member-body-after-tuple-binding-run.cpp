// [class.local]/1, [basic.def.odr]/10: a member function of a local class
// of a function template is instantiated with the enclosing function,
// as a function of its own. A tuple-like structured binding declared
// before the class is not referenced by the member's body, even when the
// body is instantiated before the binding's declaration finishes being
// processed (a coroutine collector's `pipe` binding; WP-789).
//@ expect: run
//@ std: c++20
//@ stdout: 1 3 4 3
extern "C" int printf(const char *, ...);

namespace n {
template <class T> struct pair {
  int first;
  T second;
  template <int I> constexpr auto &get() {
    if constexpr (I == 0)
      return first;
    else
      return second;
  }
};
} // namespace n

namespace std {
using size_t = decltype(sizeof 0);
template <class T> struct tuple_size;
template <size_t I, class T> struct tuple_element;
template <class T> struct tuple_size<n::pair<T>> {
  static constexpr int value = 2;
};
template <class T> struct tuple_element<0, n::pair<T>> {
  using type = int;
};
template <class T> struct tuple_element<1, n::pair<T>> {
  using type = T;
};
} // namespace std

template <class T> struct Pipe {
  T v;
};

template <class T> int mk() {
  T x = 3;
  auto [results, pipe] = n::pair<Pipe<T>>{1, {x}};
  struct S {
    explicit S(Pipe<T> p) : pipe(p) { (void)p; }
    Pipe<T> pipe;
    int twice() const { return pipe.v + 1; }
  };
  S s0(pipe);
  printf("%d %d %d ", results, s0.pipe.v, s0.twice());
  return pipe.v;
}

int main() { printf("%d\n", mk<int>()); }
