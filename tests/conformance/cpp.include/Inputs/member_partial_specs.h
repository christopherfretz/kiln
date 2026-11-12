// Partial specializations of member templates (WP-1342): of a class (a
// protected member class template with a pack, a variable template, one
// in a nested class) and of a class template (constrained, of a union,
// of a variable template, matching a template template parameter).
#ifndef MEMBER_PARTIAL_SPECS_H
#define MEMBER_PARTIAL_SPECS_H
namespace mps {
struct base {
protected:
  template <class...> struct list {};
  template <class T, class... U> struct list<T, U...> : list<U...> {
    static constexpr int size = sizeof(T);
  };
  template <class T> struct one { static constexpr int v = 1; };
  template <class T> struct one<T *> { static constexpr int v = 2; };
  template <class T> static constexpr int vt = 3;
  template <class T> static constexpr int vt<T &> = 4;

public:
  static int f();
};
struct outer {
  struct inner {
    template <class T, int N> struct arr { static constexpr int n = 0; };
    template <class T> struct arr<T, 3> { static constexpr int n = 3; };
  };
};
template <class T> struct tmpl {
  template <class U, class = void> struct ptr { static constexpr int v = 0; };
  template <class U> struct ptr<U *, void> {
    static constexpr int v = sizeof(T);
  };
  template <class U>
    requires(sizeof(U) == 8)
  struct ptr<U, void> { static constexpr int v = 8; };
  template <class U> static constexpr int vt = 1;
  template <class U> static constexpr int vt<U &> = sizeof(T) + 1;
  template <class U, bool = true> union st { U u; };
  template <class U> union st<U, false> { U u; T t; };
};
template <class A> struct rebind {
  template <template <class> class F, class T, class = void> struct p {
    using type = int;
  };
  template <template <class> class F, class T>
  struct p<F, T, decltype((void)sizeof(F<A>))> { using type = F<A>; };
};
} // namespace mps
#endif
