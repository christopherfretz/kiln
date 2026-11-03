// [temp.arg.nontype]/2-3, [temp.type]/2.5: a non-null pointer-to-member
// template argument is substituted as `&C::m` of the member's own class,
// converted to the parameter's type along the value's base/derived path
// (an implicit conversion towards the derived class, a `static_cast`
// towards a base; clang `BuildExpressionFromDeclTemplateArgument`), so a
// specialization reads, calls and compares the member it names. Two
// arguments are the same when they name the same member along the same
// path. The mangled names match clang (tests/codegen/nttp_member_pointer.cpp).
//@ std: c++20
//@ expect: run
//@ strict-diags
struct S { int v; int w; constexpr int f() const { return 7; } };
template <int S::*P> struct MP { static constexpr int get(S s) { return s.*P; } };
template <int (S::*F)() const> struct MF { static constexpr int get(S s) { return (s.*F)(); } };
static_assert(MP<&S::v>::get(S{1, 2}) == 1);
static_assert(MP<&S::w>::get(S{1, 2}) == 2);
static_assert(MF<&S::f>::get(S{1, 2}) == 7);
template <int S::*P> constexpr bool is_v = false;
template <> constexpr bool is_v<&S::v> = true;
static_assert(is_v<&S::v> && !is_v<&S::w>);
template <class T, class U> constexpr bool same = false;
template <class T> constexpr bool same<T, T> = true;
static_assert(same<MP<&S::v>, MP<&S::v> >);
static_assert(!same<MP<&S::v>, MP<&S::w> >);
static_assert(!same<MP<&S::v>, MP<nullptr> >);
template <auto P> constexpr auto id = P;
static_assert(id<&S::v> == &S::v);
static_assert(same<decltype(id<&S::v>), int S::* const>);

// Along a base/derived path.
struct B { int b; constexpr int g() const { return b + 1; } };
struct D : B { int d; };
template <int D::*P> struct X { static constexpr int get(const D &x) { return x.*P; } };
template <int (D::*F)() const> struct Y { static constexpr int get(const D &x) { return (x.*F)(); } };
template <int B::*P> struct Z { static constexpr int get(const D &x) { return x.*P; } };
constexpr int D::*pb = &B::b;
constexpr int (D::*pg)() const = &B::g;
constexpr int B::*pd = static_cast<int B::*>(&D::d);
static_assert(X<pb>::get(D{{5}, 6}) == 5);
static_assert(X<&D::d>::get(D{{5}, 6}) == 6);
static_assert(Y<pg>::get(D{{5}, 6}) == 6);
static_assert(Z<pd>::get(D{{5}, 6}) == 6);
static_assert(same<X<pb>, X<static_cast<int D::*>(&B::b)> >);
static_assert(!same<X<pb>, X<&D::d> >);
static_assert(!same<Z<pd>, Z<&B::b> >);
static_assert(id<pb> == pb && id<pd> == pd);
static_assert(same<decltype(id<pb>), int D::* const>);
template <int B::*P> int fget(const B &x) { return x.*P; }

int main() {
  S s{3, 4};
  D x{{1}, 2};
  int r = MP<&S::v>::get(s) + MF<&S::f>::get(s); // 10
  r += X<pb>::get(x) + X<&D::d>::get(x) + Y<pg>::get(x) + Z<pd>::get(x) + fget<&B::b>(x); // 8
  return r == 18 ? 0 : 1;
}
