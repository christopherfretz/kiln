// [class.static.data], [depr.static.constexpr]: `-fvisibility=hidden`
// changes the visibility of the member's definition, not its linkage
// (strong external before C++17, `weak_odr` for the redundant
// redeclaration from C++17; WP-1517). Compared with clang's IR.
//@ expect: ir
//@ std: c++14
//@ args: -fvisibility=hidden
struct A { static constexpr int n = 3; };
constexpr int A::n;
template <class T> struct B { static constexpr int n = 4; };
template <class T> constexpr int B<T>::n;
const int *use() { return &B<int>::n; }
