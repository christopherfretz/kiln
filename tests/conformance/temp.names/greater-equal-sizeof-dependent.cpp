// [temp.names]/4: `>=` inside a dependent template argument
// (`C<sizeof(T) >= 4>`) is a relational operator, also at run time in an
// instantiated function body.
//@ expect: run
//@ exit: 0
template<bool B> struct C { static constexpr bool value = B; };
template<class T> struct W { static constexpr bool big = C<sizeof(T) >= 4>::value; };
template<class... T> struct S { static constexpr bool any = C<sizeof...(T) >= 1>::value; };
template<int N> struct I { static constexpr int v = N; };
constexpr int k = 3;
static_assert(W<int>::big && !W<char>::big);
static_assert(S<int>::any && !S<>::any);
static_assert(I<(k >= 2)>::v == 1);
static_assert(C<k >= 2>::value);
template<class T> int f() { return C<sizeof(T) >= 4>::value ? 1 : 2; }
int main() { return f<int>() + f<char>() - 3; }
