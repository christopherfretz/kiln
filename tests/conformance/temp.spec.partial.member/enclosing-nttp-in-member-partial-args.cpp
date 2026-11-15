// [temp.spec.partial.member]/2, [temp.spec.partial.match]/2: a partial
// specialization of a member template whose arguments name a non-type
// parameter of the enclosing class template (`inner<C, S>` in
// `outer<bool C>`). Instantiating `outer<true>` redeclares it as
// `inner<true, S>`, and `inner<true, false>` must match it. kiln kept the
// substituted `C` as an unconverted expression argument, so the partial
// never matched and the primary was chosen silently (found running GCC's
// C++ testsuite). Siblings: an `int` and an enumeration parameter, a
// class-type one, and the same through a variable template.
//@ expect: compile-ok
//@ strict-diags
//@ std: c++20
enum E { e0, e1 };
struct L {
  int v;
};

template <bool C>
struct outer {
  template <bool W, bool S>
  struct inner {
    static constexpr int v = 0;
  };
  template <bool S>
  struct inner<C, S> {
    static constexpr int v = 1;
  };
};
static_assert(outer<true>::inner<true, false>::v == 1);
static_assert(outer<true>::inner<true, true>::v == 1);
static_assert(outer<true>::inner<false, false>::v == 0);
static_assert(outer<false>::inner<false, true>::v == 1);
static_assert(outer<false>::inner<true, true>::v == 0);

template <int N, E K>
struct o_int {
  template <int W, E V, class T>
  struct in {
    static constexpr int v = 0;
  };
  template <class T>
  struct in<N, K, T> {
    static constexpr int v = 1;
  };
  template <E V>
  struct in<N + 1, V, int> {
    static constexpr int v = 2;
  };
};
static_assert(o_int<3, e1>::in<3, e1, char>::v == 1);
static_assert(o_int<3, e1>::in<3, e0, char>::v == 0);
static_assert(o_int<3, e1>::in<4, e0, int>::v == 2);
static_assert(o_int<3, e1>::in<4, e0, long>::v == 0);

template <L X>
struct o_class {
  template <L Y, class T>
  struct in {
    static constexpr int v = 0;
  };
  template <class T>
  struct in<X, T> {
    static constexpr int v = 1;
  };
};
static_assert(o_class<L{7}>::in<L{7}, int>::v == 1);
static_assert(o_class<L{7}>::in<L{8}, int>::v == 0);

template <bool C>
struct o_var {
  template <bool W, bool S>
  static constexpr int v = 0;
  template <bool S>
  static constexpr int v<C, S> = 1;
};
static_assert(o_var<true>::v<true, false> == 1);
static_assert(o_var<true>::v<false, false> == 0);

