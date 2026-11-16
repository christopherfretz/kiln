// [class.static.data]/3 in C++14 (N4140 9.4.2/3): the in-class
// declaration of a `constexpr` static data member is not a definition;
// the out-of-class one is, a strong external one emitted even when
// nothing uses it ([basic.def]/2, [basic.def.odr]). Without it, an
// odr-use names the external symbol (clang: an `available_externally`
// constant holding the in-class value), never a private copy. Before
// WP-1517 kiln treated the member as inline in every mode
// (`linkonce_odr` in a comdat, dropped when unused), so a duplicate
// definition linked silently and a unit relying on another's definition
// could find none (reported from linking kiln objects with mortar).
// Templates keep their instantiation linkage. Compared with clang's IR.
//@ expect: ir
//@ std: c++14
struct A { static constexpr int n = 3; };
constexpr int A::n;

struct Unused { static constexpr int n = 4; };
constexpr int Unused::n;

struct NoDef { static constexpr int n = 5; };

struct C { static const int n = 6; };
const int C::n;

struct L { int a; constexpr L(int x) : a(x) {} };
struct E { static constexpr L l{7}; };
constexpr L E::l;

struct __attribute__((visibility("hidden"))) G { static constexpr int n = 8; };
constexpr int G::n;

namespace { struct F { static constexpr int n = 9; }; constexpr int F::n; }

struct I1 { static constexpr int n = 10; };
alignas(16) constexpr int I1::n;
struct I2 { static constexpr int n = 11; };
[[gnu::used]] constexpr int I2::n;
struct I3 { static constexpr int n = 12; };
__attribute__((weak)) constexpr int I3::n;

template <class T> struct B { static constexpr int n = 13; };
template <class T> constexpr int B<T>::n;
template struct B<long>;
template <class T> struct BN { static constexpr int n = 14; };

struct V { template <class T> static constexpr int n = sizeof(T); };
template <class T> constexpr int V::n;

const int *use() {
  static const int *ps[] = {&A::n, &NoDef::n, &C::n, &F::n, &B<int>::n,
                            &BN<int>::n, &V::n<int>};
  return ps[0];
}
