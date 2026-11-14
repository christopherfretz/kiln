// Debug info (WP-1400): class and function template specialization names
// under clang's debug-information printing policy: every converted
// argument, defaults included (`str<char, n::v1::traits<char>,
// n::v1::al<char> >`); inline namespaces kept in the scope of a name
// inside the arguments (`n::v1::`); an enumeration value as a cast
// (`lk<(n::Pol)2>`, `slk<(n::SPol)1>`); an integer argument with its
// literal suffix (`arr<int, 3UL>`). clang 19 and 22 agree.
//@ args: -g
namespace n {
inline namespace v1 {
template <class T> struct traits {};
template <class T> struct al {};
template <class C, class Tr = traits<C>, class A = al<C>> struct str { C c; };
} // namespace v1
enum Pol { P0, P1, P2 };
enum class SPol { Q0, Q1 };
template <Pol P = P2> struct lk { int x; };
template <SPol P> struct slk { int x; };
template <class T, unsigned long N = 3> struct arr { T t[N]; };
template <class T, int N = -2, char C = 'c', bool B = true> struct mix { T t; };
} // namespace n
n::str<char> s;
n::str<n::str<char>> s2;
n::lk<> l;
n::lk<n::P0> l0;
n::slk<n::SPol::Q1> sl;
n::arr<int> a;
n::mix<long> m;
template <class T, class U = n::str<T>> U mk(T) { return U(); }
template <n::Pol P, unsigned long N> int at() { return P + N; }
void use() {
  mk('c');
  at<n::P1, 7>();
}
