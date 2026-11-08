// [temp.over.link]/5, [temp.constr.order], P2996 [temp.type]: a
// dependent splice written the same way in two templates is equivalent,
// so redeclarations match and the overload adding a conjunct to a
// concept-id whose mapping holds the splice is more constrained. Covers
// an expression splice `[: R :]`, a splice template-id
// `template [: R :] < int >`, a type splice `typename [: R :]` in a
// parameter type, and a member splice `s.[: R :]` in a trailing return
// type. Compared with GCC 16.1 (clang has no reflection). WP-884.
//@ expect: run
//@ oracle: gcc
//@ args: -freflection
//@ skip(clang): clang 19 does not implement P2996 reflection
using info = decltype(^^int);
template <int N> concept P = N > 0;
template <class T> concept C = sizeof(T) > 0;
constexpr int one = 1;
struct S { int m; static constexpr int sm = 2; };
template <class T> constexpr int vt = sizeof(T);

// subsumption through an expression splice in a concept-id's mapping
template <info R> requires P<([:R:])> constexpr int f1() { return 1; }
template <info R> requires P<([:R:])> && true constexpr int f1() { return 2; }
static_assert(f1<^^one>() == 2);


// subsumption through a splice template-id
template <info R> requires P<(template [:R:]<int>)> constexpr int f3() { return 1; }
template <info R> requires P<(template [:R:]<int>)> && true constexpr int f3() { return 2; }
static_assert(f3<^^vt>() == 2);

// redeclarations
template <info R> int g1() requires P<([:R:])>;
template <info R> int g1() requires P<([:R:])> { return 3; }
template <info R> auto g2() -> decltype([:R:]);
template <info R> auto g2() -> decltype([:R:]) { return [:R:]; }
template <info R> void g3(typename [:R:]);
template <info R> void g3(typename [:R:]) {}
template <info R> auto g4(S s) -> decltype(s.[:R:]);
template <info R> auto g4(S s) -> decltype(s.[:R:]) { return s.[:R:]; }
template <info R> int g5(int (*)[template [:R:]<int>]);
template <info R> int g5(int (*)[template [:R:]<int>]) { return 5; }
int main() {
  int a[4];
  return g1<^^one>() + g2<^^one>() + g4<^^S::m>(S{0}) + g5<^^vt>(&a) - 9;
}
