// [temp.inst]/4, [temp.dep.type]/1: boost::variant<boost::blank, A, B>
// without boost. The storage's alignment is a fold of boost.mpl's
// `add_alignment` (`size_t<lcm<State::value, alignment_of<Item>::value>>`)
// over the alternatives, and `type_with_alignment<Align>` names the
// non-dependent `alignment_of<char>::value` in its definition. With
// `integral_constant<T, val>::value` defined out of class (boost's
// type_traits), that reference instantiates the member's definition while
// a template is being defined; the definition must not be dependent, or
// the fold's `lcm<...>` stays dependent after substitution and the
// variant has no storage type.
//@ expect: run
//@ strict-diags
//@ stdout: 8 16 | 0 blank | 1 int 42 | 2 B 7 2.5
extern "C" int printf(const char *, ...);
typedef unsigned long size_t;

template <class T, T val> struct integral_constant {
  static const T value = val;
  typedef integral_constant type;
};
template <class T, T val> T const integral_constant<T, val>::value;
template <class T> struct alignment_of : integral_constant<size_t, alignof(T)> {};

template <size_t Target, bool check> struct char_alignment { typedef char type; };
template <size_t Target> struct char_alignment<Target, false> {
  struct alignas(Target) type { char c; };
};
template <size_t Align> struct type_with_alignment {
  typedef typename char_alignment<Align, alignment_of<char>::value >= Align>::type type;
};

namespace mpl {
template <size_t N> struct size_t_ { static const size_t value = N; typedef size_t_ type; };
template <size_t A, size_t B> struct gcd : gcd<B, A % B> {};
template <size_t A> struct gcd<A, 0> { static const size_t value = A; };
template <size_t A, size_t B> struct lcm { static const size_t value = A / gcd<A, B>::value * B; };
template <class F, class A1, class A2> struct apply2 : F::template apply<A1, A2> {};
template <class F, class State, class... T> struct fold { typedef State type; };
template <class F, class State, class H, class... T> struct fold<F, State, H, T...>
    : fold<F, typename apply2<F, State, H>::type, T...> {};
template <class State, class... T> struct max_size { typedef State type; };
template <class State, class H, class... T> struct max_size<State, H, T...>
    : max_size<size_t_<(State::value > sizeof(H) ? State::value : sizeof(H))>, T...> {};
} // namespace mpl

struct add_alignment {
  template <typename State, typename Item>
  struct apply : mpl::size_t_<mpl::lcm<State::value, alignment_of<Item>::value>::value> {};
};

struct blank {};
struct B { int i; double d; };

template <class... T> class variant {
  static const size_t align = mpl::fold<add_alignment, mpl::size_t_<1>, T...>::type::value;
  static const size_t size = mpl::max_size<mpl::size_t_<1>, T...>::type::value;
  union {
    typename type_with_alignment<align>::type aligner_;
    unsigned char buf_[size];
  };
  int which_;

public:
  variant() : which_(0) {}
  template <class U> variant(int w, const U &u) : which_(w) { __builtin_memcpy(buf_, &u, sizeof u); }
  int which() const { return which_; }
  static size_t storage_align() { return alignof(typename type_with_alignment<align>::type); }
  template <class V> void apply_visitor(V v) const;
};

template <class T> T load(const unsigned char *p) { T t; __builtin_memcpy(&t, p, sizeof t); return t; }

template <class... T> template <class V> void variant<T...>::apply_visitor(V v) const {
  switch (which_) {
  case 0: v(blank()); break;
  case 1: v(load<int>(buf_)); break;
  case 2: v(load<B>(buf_)); break;
  }
}

struct printer {
  void operator()(blank) const { printf("blank"); }
  void operator()(int i) const { printf("int %d", i); }
  void operator()(const B &b) const { printf("B %d %.1f", b.i, b.d); }
};

typedef variant<blank, int, B> V;

int main() {
  printf("%lu %lu", V::storage_align(), sizeof(V) - alignof(V));
  V vs[] = {V(), V(1, 42), V(2, B{7, 2.5})};
  for (const V &v : vs) {
    printf(" | %d ", v.which());
    v.apply_visitor(printer());
  }
  printf("\n");
}
