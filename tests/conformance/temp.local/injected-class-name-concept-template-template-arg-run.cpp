// [temp.local]/1: the injected-class-name of a class template used as
// the argument of a concept's template template parameter names the
// class template (`requires not_inst<A, alloc>` inside `alloc<T>`, a
// type-erased allocator's converting constructor; WP-789).
//@ expect: run
//@ std: c++20
//@ stdout: 1 0 7 2
extern "C" int printf(const char *, ...);

template <class, template <class...> class> constexpr bool is_inst = false;
template <class... As, template <class...> class T>
constexpr bool is_inst<T<As...>, T> = true;
template <class A, template <class...> class T>
concept not_inst = !is_inst<A, T>;

template <class T> struct alloc {
  int from = 0;
  static constexpr bool other = not_inst<int, alloc>;
  static constexpr bool self = not_inst<alloc<long>, alloc>;
  alloc() = default;
  alloc(const alloc &o) : from(o.from + 1) {}
  template <class A>
    requires not_inst<A, alloc>
  alloc(A a) : from(a) {}
};

int main() {
  alloc<int> x{7};
  alloc<int> y{x};
  alloc<int> z{y};
  printf("%d %d %d %d\n", alloc<int>::other, alloc<int>::self, x.from, z.from - x.from);
}
