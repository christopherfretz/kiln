// Debug info (WP-814): template template arguments (single, packed, from a
// namespace) and a null pointer argument (`i8 0`), as clang 19.
//@ args: -g
namespace ns {
template <class T> struct Box { T v; };
}
template <class T> struct Plain { T v; };
template <template <class> class C, class T> struct Holder { C<T> c; };
template <template <class> class... Cs> struct Many { int n; };
template <decltype(nullptr) P> struct NullArg { int v; };
Holder<ns::Box, int> h1;
Holder<Plain, long> h2;
Many<Plain, ns::Box> m;
NullArg<nullptr> na;
int main() { return static_cast<int>(h1.c.v + h2.c.v + m.n + na.v); }
