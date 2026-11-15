// Debug info (WP-1445): an alias template specialization's typedef is
// named by its arguments as written, with a written argument equal to
// its default kept and an omitted one left out, in and out of templates
// (clang 19 and 22 agree).
//@ args: -g
template <class T, int N = 2> struct B { T v[N]; };
namespace n {
template <class T, int N = 2> using A = B<T, N>;
}
template <class T, int N = sizeof(T)> using AS = B<T, N>;
template <class T, class U = T *> struct D { T t; U u; };
template <class T, class U = T *> using AD = D<T, U>;
template <int N = 2> using AN = B<char, N>;
n::A<short, 2> a1;
n::A<short> a2;
AS<short, 2> as1;
AS<short> as2;
AD<short, short *> ad;
AN<2> an;
AN<> an0;
n::A<short, 1 + 1> a3;
template <class T> struct W { n::A<T> m; n::A<T, 3> m3; n::A<T, 2> m2; };
W<long> w;
template <class T> int g(n::A<T> x) { return x.v[0]; }
int u = g<int>({});
