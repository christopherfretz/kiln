// Debug info (WP-1414): `defaulted: true` on every template argument
// taken from or equal to its parameter's default, trailing or not
// (clang's `TemplateArgument::getIsDefaulted`, set by
// `CheckTemplateArgumentList`; [temp.arg.general], [temp.param]/10):
// `S<int, char>` has `A` and `N` defaulted, `S<int, long, 4>` `A` and
// `B`; the arguments of an explicit specialization ([temp.expl.spec]) and
// of a variable template specialization the same way. A pack and a
// function template's arguments never are. clang 19 and 22 agree.
//@ args: -g
template <class A = int, class B = long, int N = 3> struct S { int k = 0; };
S<int, char> s1;
S<int, long, 4> s2;
S<> s3;
S<char> s4;
template <> struct S<short, long, 3> { int k = 1; };
S<short> s5;
template <> struct S<int, long, 5> { int k = 2; };
S<int, long, 5> s6;
template <class T, class U = T> struct D { int k = 0; };
D<int> d1;
D<int, char> d2;
template <class T, class... P> struct Pk { int k = 0; };
Pk<int> pk;
template <class T, class U = int> T v = T();
template <int A = 1, int B = 2> int f() { return A + B; }
int use() {
  return s1.k + s2.k + s3.k + s4.k + s5.k + s6.k + d1.k + d2.k + pk.k + v<int> + v<long, char> +
         f<1, 3>() + f<>();
}
