// [temp.constr.order]/1: Z subsumes Majority8 and Y does not subsume Z, so
// foo(Z auto) is the most constrained; the normal form is large (clang
// GH122581; clang 19 runs out of memory). From clang's SemaCXX/concepts-
// subsumption.cpp. Found by the clang negative-test run (WP-912).
//@ std: c++20
//@ requires: clang>=22
//@ expect: compile-ok
template <typename T> concept C0 = true;
template <typename T> concept C1 = true;
template <typename T> concept C2 = true;
template <typename T> concept C3 = true;
template <typename T> concept C4 = true;
template <typename T> concept C5 = true;
template <typename T> concept C6 = true;
template <typename T> concept C7 = true;
template <typename T> concept C8 = true;
template <typename T> concept C9 = true;

template <typename T>
concept Majority8 =
    (C0<T> && C1<T> && C2<T> && C3<T>) ||
    (C0<T> && C1<T> && C2<T> && C4<T>) ||
    (C0<T> && C1<T> && C2<T> && C5<T>) ||
    (C0<T> && C1<T> && C2<T> && C6<T>) ||
    (C0<T> && C1<T> && C2<T> && C7<T>) ||
    (C0<T> && C1<T> && C3<T> && C4<T>) ||
    (C0<T> && C1<T> && C3<T> && C5<T>) ||
    (C0<T> && C1<T> && C3<T> && C6<T>) ||
    (C0<T> && C1<T> && C3<T> && C7<T>) ||
    (C0<T> && C1<T> && C4<T> && C5<T>) ||
    (C0<T> && C1<T> && C4<T> && C6<T>) ||
    (C0<T> && C1<T> && C4<T> && C7<T>) ||
    (C0<T> && C1<T> && C5<T> && C6<T>) ||
    (C0<T> && C1<T> && C5<T> && C7<T>) ||
    (C0<T> && C1<T> && C6<T> && C7<T>) ||
    (C0<T> && C2<T> && C3<T> && C4<T>) ||
    (C0<T> && C2<T> && C3<T> && C5<T>) ||
    (C0<T> && C2<T> && C3<T> && C6<T>) ||
    (C0<T> && C2<T> && C3<T> && C7<T>) ||
    (C0<T> && C2<T> && C4<T> && C5<T>) ||
    (C0<T> && C2<T> && C4<T> && C6<T>) ||
    (C0<T> && C2<T> && C4<T> && C7<T>) ||
    (C0<T> && C2<T> && C5<T> && C6<T>) ||
    (C0<T> && C2<T> && C5<T> && C7<T>) ||
    (C0<T> && C2<T> && C6<T> && C7<T>) ||
    (C0<T> && C3<T> && C4<T> && C5<T>) ||
    (C0<T> && C3<T> && C4<T> && C6<T>) ||
    (C0<T> && C3<T> && C4<T> && C7<T>) ||
    (C0<T> && C3<T> && C5<T> && C6<T>) ||
    (C0<T> && C3<T> && C5<T> && C7<T>) ||
    (C0<T> && C3<T> && C6<T> && C7<T>) ||
    (C0<T> && C4<T> && C5<T> && C6<T>) ||
    (C0<T> && C4<T> && C5<T> && C7<T>) ||
    (C0<T> && C4<T> && C6<T> && C7<T>) ||
    (C0<T> && C5<T> && C6<T> && C7<T>) ||
    (C1<T> && C2<T> && C3<T> && C4<T>) ||
    (C1<T> && C2<T> && C3<T> && C5<T>) ||
    (C1<T> && C2<T> && C3<T> && C6<T>) ||
    (C1<T> && C2<T> && C3<T> && C7<T>) ||
    (C1<T> && C2<T> && C4<T> && C5<T>) ||
    (C1<T> && C2<T> && C4<T> && C6<T>) ||
    (C1<T> && C2<T> && C4<T> && C7<T>) ||
    (C1<T> && C2<T> && C5<T> && C6<T>) ||
    (C1<T> && C2<T> && C5<T> && C7<T>) ||
    (C1<T> && C2<T> && C6<T> && C7<T>) ||
    (C1<T> && C3<T> && C4<T> && C5<T>) ||
    (C1<T> && C3<T> && C4<T> && C6<T>) ||
    (C1<T> && C3<T> && C4<T> && C7<T>) ||
    (C1<T> && C3<T> && C5<T> && C6<T>) ||
    (C1<T> && C3<T> && C5<T> && C7<T>) ||
    (C1<T> && C3<T> && C6<T> && C7<T>) ||
    (C1<T> && C4<T> && C5<T> && C6<T>) ||
    (C1<T> && C4<T> && C5<T> && C7<T>) ||
    (C1<T> && C4<T> && C6<T> && C7<T>) ||
    (C1<T> && C5<T> && C6<T> && C7<T>) ||
    (C2<T> && C3<T> && C4<T> && C5<T>) ||
    (C2<T> && C3<T> && C4<T> && C6<T>) ||
    (C2<T> && C3<T> && C4<T> && C7<T>) ||
    (C2<T> && C3<T> && C5<T> && C6<T>) ||
    (C2<T> && C3<T> && C5<T> && C7<T>) ||
    (C2<T> && C3<T> && C6<T> && C7<T>) ||
    (C2<T> && C4<T> && C5<T> && C6<T>) ||
    (C2<T> && C4<T> && C5<T> && C7<T>) ||
    (C2<T> && C4<T> && C6<T> && C7<T>) ||
    (C2<T> && C5<T> && C6<T> && C7<T>) ||
    (C3<T> && C4<T> && C5<T> && C6<T>) ||
    (C3<T> && C4<T> && C5<T> && C7<T>) ||
    (C3<T> && C4<T> && C6<T> && C7<T>) ||
    (C3<T> && C5<T> && C6<T> && C7<T>) ||
    (C4<T> && C5<T> && C6<T> && C7<T>);

template <typename T>concept Y = C0<T> || Majority8<T>;
template <typename T>concept Z = Majority8<T> && C1<T>;

constexpr int foo(Majority8 auto x) { return 10; }
constexpr int foo(Y auto y) { return 20; }
constexpr int foo(Z auto y) { return 30; }
static_assert(foo(0) == 30);
