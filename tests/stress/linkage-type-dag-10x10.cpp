// [basic.link]/15 (a type has linkage through its template arguments),
// [temp.arg.type], spec/abi.md §8.22: `A9` names 10^9 paths through ten
// shared nodes and `B6` about 10^7, so the linkage of `f`'s parameter type
// (internal or not, clang `getLVForTemplateArgumentList`) must be computed
// once per type, not once per path. Before WP-1076 this compile ran for
// hours. Reduced from clang's mangle-ms-back-references-pr13207.cpp,
// mangled with the Itanium ABI.
//@ expect: run
//@ mem<=512
template <class T1, class T2, class T3, class T4, class T5, class T6, class T7,
          class T8, class T9, class T10>
struct Fooob {};

using A0 = Fooob<int, int, int, int, int, int, int, int, int, int>;
using A1 = Fooob<A0, A0, A0, A0, A0, A0, A0, A0, A0, A0>;
using A2 = Fooob<A1, A1, A1, A1, A1, A1, A1, A1, A1, A1>;
using A3 = Fooob<A2, A2, A2, A2, A2, A2, A2, A2, A2, A2>;
using A4 = Fooob<A3, A3, A3, A3, A3, A3, A3, A3, A3, A3>;
using A5 = Fooob<A4, A4, A4, A4, A4, A4, A4, A4, A4, A4>;
using A6 = Fooob<A5, A5, A5, A5, A5, A5, A5, A5, A5, A5>;
using A7 = Fooob<A6, A6, A6, A6, A6, A6, A6, A6, A6, A6>;
using A8 = Fooob<A7, A7, A7, A7, A7, A7, A7, A7, A7, A7>;
using A9 = Fooob<A8, A8, A8, A8, A8, A8, A8, A8, A8, A8>;

template <class T1, class T2, class T3, class T4, class T5, class T6, class T7,
          class T8, class T9, class T10, class T11, class T12, class T13,
          class T14, class T15, class T16, class T17, class T18, class T19,
          class T20>
struct Food {};

using B0 = Food<int, int, int, int, int, int, int, int, int, int,
                int, int, int, int, int, int, int, int, int, int>;
using B1 = Food<B0, B0, B0, B0, B0, B0, B0, B0, B0, B0,
                B0, B0, B0, B0, B0, B0, B0, B0, B0, B0>;
using B2 = Food<B1, B0, B0, B0, B0, B0, B0, B0, B0, B0,
                B1, B1, B1, B1, B1, B1, B1, B1, B1, B1>;
using B3 = Food<B2, B1, B0, B0, B0, B0, B0, B0, B0, B0,
                B2, B2, B2, B2, B2, B2, B2, B2, B2, B2>;
using B4 = Food<B3, B2, B1, B0, B0, B0, B0, B0, B0, B0,
                B3, B3, B3, B3, B3, B3, B3, B3, B3, B3>;
using B5 = Food<B4, B3, B2, B1, B0, B0, B0, B0, B0, B0,
                B4, B4, B4, B4, B4, B4, B4, B4, B4, B4>;
using B6 = Food<B5, B4, B3, B2, B1, B0, B0, B0, B0, B0,
                B5, B5, B5, B5, B5, B5, B5, B5, B5, B5>;

int f(A9) { return 1; }
int f(B6) { return 2; }

int main() { return f(A9{}) + f(B6{}) == 3 ? 0 : 1; }
