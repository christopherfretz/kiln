// [stmt.if]/2, [basic.def.odr]/5: an implicitly-declared or defaulted
// special member named in a discarded statement is not odr-used, so it is
// not implicitly defined ([class.copy.ctor]/12, [class.copy.assign]/10,
// [class.default.ctor]/4, [class.dtor]/11), and the member functions of
// the class template its definition would call are not instantiated
// ([temp.inst]/8): `W<int>`'s copy constructor, copy assignment, default
// constructor and destructor all static_assert. Only a constexpr one
// would be defined, for constant evaluation (clang
// `isImplicitlyDefinableConstexprFunction`); before C++23 (P2448R2) none
// of `B`'s, not even the destructor, is constexpr as `W<int>`'s are not
// (discarded-statement-still-instantiates covers the C++23 case).
//@ expect: run
//@ std: c++20
//@ exit: 3
template <class T> inline constexpr bool never = false;
template <class T> struct W {
  W() { static_assert(never<T>, "W() instantiated"); }
  W(const W &) { static_assert(never<T>, "W(const W&) instantiated"); }
  W &operator=(const W &) {
    static_assert(never<T>, "W::operator= instantiated");
    return *this;
  }
  ~W() { static_assert(never<T>, "~W instantiated"); }
};
struct B {
  W<int> w;
};
struct C {
  W<long> w;
  C() = default;
  C(const C &) = default;
  C &operator=(const C &) = default;
  ~C() = default;
};
int use(B *p, C *q) {
  if constexpr (false) {
    B b;
    (void)B(*p);
    b = *p;
    C c;
    (void)C(*q);
    c = *q;
  }
  return 3;
}
int main() { return use(nullptr, nullptr); }
