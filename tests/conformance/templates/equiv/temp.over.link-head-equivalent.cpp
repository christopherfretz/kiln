// [temp.over.link]/6: equivalent template heads (parameter names, and
// expressions in constraints spelled with other names, do not matter).
template <class T> concept K = sizeof(T) > 1;
template <int N> struct A;
template <int M> struct A { static constexpr int v = M; };
template <K T> struct B;
template <K U> struct B { static constexpr int v = 1; };
template <class T> requires (sizeof(T) > 1) struct C;
template <class U> requires (sizeof(U) > 1) struct C { static constexpr int v = 2; };
template <template <class> class P, class... Ts> struct D;
template <template <class> class Q, class... Us> struct D { static constexpr int v = 3; };
template <class T> requires K<T> using E = T;
template <class U> requires K<U> using E = U;
template <class T> struct W {};
static_assert(A<4>::v + B<int>::v + C<int>::v + D<W>::v == 10);
static_assert(sizeof(E<int>) == sizeof(int));
