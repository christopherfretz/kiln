//@ expect: compile-ok
//@ requires: clang>=22
//@ strict-diags
// [expr.const.init]/2 and [dcl.struct.bind] (C++26, P2686R5): a structured
// binding's address is a permitted result exactly when the member access
// it denotes is: bindings of objects with static storage duration
// (namespace scope, static, static constexpr; by value, by reference,
// tuple-like, array), constexpr bindings and their values, bit-field
// bindings read by value, and any binding inside a constant evaluation
// (the object is within the evaluation). The automatic forms clang 22
// rejects: p2686-referenceable-automatic.cpp.
namespace std {
using size_t = decltype(sizeof 0);
template <class T> struct tuple_size;
template <size_t I, class T> struct tuple_element;
template <class T> requires requires { tuple_size<T>::value; }
struct tuple_size<const T> { static constexpr size_t value = tuple_size<T>::value; };
template <size_t I, class T> struct tuple_element<I, const T> { using type = const typename tuple_element<I, T>::type; };
} // namespace std
struct S { int x, y; };
struct B { int a : 4, b : 4; };
class T {
  int x_, y_;
public:
  constexpr T(int a, int b) : x_(a), y_(b) {}
  constexpr const int &y() const { return y_; }
  template <std::size_t I> constexpr const int &get() const { return I ? y_ : x_; }
};
template <> struct std::tuple_size<T> { static constexpr size_t value = 2; };
template <std::size_t I> struct std::tuple_element<I, T> { using type = const int; };
struct X { template <std::size_t I> constexpr int get() const { return int(I) + 10; } };
template <> struct std::tuple_size<X> { static constexpr size_t value = 2; };
template <std::size_t I> struct std::tuple_element<I, X> { using type = int; };
template <const int &R> constexpr int value_of = R;

constexpr S gs{1, 2};
S ms{3, 4};
T mt{5, 6};
int marr[2] = {7, 8};

// Namespace scope: static storage duration, referenceable from anywhere.
auto [na, nb] = ms;
constexpr int *pnb = &nb;
static_assert(pnb == &na + 1);
constexpr auto [ca, cb] = gs;
constexpr const int *pcb = &cb;
static_assert(*pcb == 2 && cb == 2 && value_of<cb> == 2);
auto &[ra, rb] = ms;
static_assert(&rb == &ms.y);
auto &[ta, tb] = mt;
static_assert(&tb == &mt.y());
auto [ea, eb] = marr;
static_assert(&eb == &ea + 1);
constexpr auto [xa, xb] = X{};
static_assert(xa == 10 && xb == 11);
constexpr B gb{1, 2};
constexpr auto [ba, bb] = gb;
static_assert(ba == 1 && bb == 2);

void f() {
  static auto [sa, sb] = ms;
  static_assert(&sb == &sa + 1);
  static constexpr auto [sca, scb] = gs;
  static constexpr const int *p = &scb;
  static_assert(*p == 2 && value_of<scb> == 2);
  auto &[fa, fb] = mt;
  constexpr const int *q = &fb;
  static_assert(q == &mt.y());
  constexpr auto [la, lb] = gs;
  static_assert(la + lb == 3);
  auto lambda = [=] { constexpr int v = lb; return v; };
  (void)lambda;
}

constexpr int g() {
  S s{1, 2};
  auto [a, b] = s;
  int *pb = &b;
  *pb = 5;
  auto &[c, d] = s;
  return s.y + b + d;
}
static_assert(g() == 9);
