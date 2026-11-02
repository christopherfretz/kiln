// [over.match.class.deduct]/1.2, /3 (P1814): the guide of a constructor
// template with template parameters beyond the class's, deduced through an
// alias template. Its signature and template parameters may name members of
// the class template (a member class, a member alias template, a static
// member function template of a member class); substituting the alias's deductions
// leaves the class dependent (`P<T, short>`), so those stay members of an
// unknown specialization ([temp.dep.type]/6) until the guide is used. The
// libc++ `std::pair` shape: `__enable_if_t<_CheckArgs::template ...>`.
//@ expect: run
template <bool B, class T = int> struct enable_if {};
template <class T> struct enable_if<true, T> { using type = T; };
template <class... T> struct Tup {};
template <unsigned long... I> struct Idx {};

template <class T1, class T2> struct P {
  struct Chk {
    static constexpr bool ok() { return sizeof(T1) > 1; }
    template <class U> static constexpr bool same() { return __is_same(U, T2); }
  };
  template <bool D> using ChkDep = Chk;
  T1 first;
  T2 second;
  int kind = 0;
  template <bool D = true, typename enable_if<ChkDep<D>::ok(), int>::type = 0>
  constexpr P(T1 const& a, T2 const& b) : first(a), second(b), kind(1) {}
  template <class U, typename enable_if<Chk::template same<U>(), int>::type = 0>
  constexpr P(int, U const& b, T1 const& a) : first(a), second(b), kind(2) {}
  template <class... A, unsigned long... I>
  constexpr P(Tup<A...>, Idx<I...>, T1 const& a) : first(a), second(), kind(3) {}
};

// Through the class template itself ([over.match.class.deduct]/1.2).
constexpr P p0(1.5, (short)2);
static_assert(__is_same(decltype(p0), const P<double, short>) && p0.kind == 1);

// Through an alias template ([over.match.class.deduct]/3).
template <class T> using PS = P<T, short>;
constexpr PS p1(1.5, (short)2);
static_assert(__is_same(decltype(p1), const P<double, short>) && p1.kind == 1);
constexpr PS p2(0, (short)3, 2.5);
static_assert(__is_same(decltype(p2), const P<double, short>) && p2.kind == 2);
constexpr PS p3(Tup<int, char>(), Idx<1, 2>(), 4L);
static_assert(__is_same(decltype(p3), const P<long, short>) && p3.kind == 3);

template <class T> using PD = P<double, T>;
constexpr PD p5(2.0, 'c');
static_assert(__is_same(decltype(p5), const P<double, char>) && p5.kind == 1);

int main() { return p1.second + p2.kind + p3.kind - 7; }
