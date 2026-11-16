// [dcl.constexpr]/1, [dcl.inline] (C++17): a `constexpr` static data
// member is implicitly inline, defined in its class (`linkonce_odr`,
// emitted when used). The out-of-class redeclaration `constexpr int
// A::n;` is redundant and deprecated ([depr.static.constexpr]); clang
// gives the member it redeclares `weak_odr` linkage and emits it even
// when unused (`getInlineVariableDefinitionKind` → `Strong`), so a unit
// built as C++14, which refers to the member without defining it,
// links (WP-1517). A member declared `inline` itself, or without the
// redeclaration, stays discardable; templates keep their instantiation
// linkage, an explicit specialization is `weak_odr`. Compared with
// clang's IR.
//@ expect: ir
//@ std: c++17
struct A { static constexpr int n = 3; };
constexpr int A::n;

struct Unused { static constexpr int n = 4; };
constexpr int Unused::n;

struct NoDef { static constexpr int n = 5; };
struct NoDefUnused { static constexpr int n = 6; };

struct C { static const int n = 7; };
const int C::n;

struct D { static inline int n = 8; static inline constexpr int m = 9; };
constexpr int D::m;

struct L { int a; constexpr L(int x) : a(x) {} };
struct E { static constexpr L l{10}; };
constexpr L E::l;

struct __attribute__((visibility("hidden"))) G { static constexpr int n = 11; };
constexpr int G::n;

namespace { struct F { static constexpr int n = 12; }; constexpr int F::n; }

struct I1 { alignas(16) static constexpr int n = 13; };
constexpr int I1::n;

template <class T> struct B { static constexpr int n = 14; };
template <class T> constexpr int B<T>::n;
template struct B<long>;
template <> constexpr int B<char>::n = 15;

struct V { template <class T> static constexpr int n = sizeof(T); };
template <class T> constexpr int V::n;

const int *use() {
  static const int *ps[] = {&A::n, &NoDef::n, &C::n, &D::m, &F::n, &B<int>::n,
                            &V::n<int>};
  return ps[0];
}
int *use_d() { return &D::n; }
