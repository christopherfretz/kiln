// [temp.constr.atomic]/1-3: an atom's parameter mapping (and so its
// satisfaction-cache key) includes every template parameter named inside
// a lambda-expression, a statement expression, a local class or enum, a
// default argument or a requires-clause there. Checking `C<int, long>`
// first must not answer `C<int, char>` from the cache when only `U`
// differs. The note names the concept-id and the substituted atom, as
// clang 19 prints them. WP-821.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 53:15 error "static assertion failed"
//@ diag: 53:15 note "because 'M<int, char>' evaluated to false"
//@ diag: 49:75 note "because 'sizeof(char) > 1' (1 > 1) evaluated to false"
template <class T, class U> concept C1 = [] { return sizeof(U) > 1; }();
static_assert(C1<int, long> && !C1<int, char>);

template <class T, class U> concept C2 = [] { return ({ U u{}; sizeof(u); }) > 1; }();
static_assert(C2<int, long> && !C2<int, char>);

template <class T, class U> concept C3 = [n = sizeof(U)] { return n > 1; }();
static_assert(C3<int, long> && !C3<int, char>);

template <class T, class U>
concept C4 = [](int n = sizeof(U)) { return n > 1; }();
static_assert(C4<int, long> && !C4<int, char>);

// (U enters the key through the requires-clause; calling the lambda is
// the separate xfail expr.prim.lambda.closure/trailing-requires-call.cpp)
template <class T, class U>
concept C5 = requires { [](int) requires(sizeof(U) > 1) {}; };
static_assert(C5<int, long> && C5<int, char>);

template <class T, class U>
concept C6 = [] { enum E { e = sizeof(U) }; return e > 1; }();
static_assert(C6<int, long> && !C6<int, char>);

template <class T, class U>
concept C7 = [] { struct S { U u; }; return sizeof(S) > 1; }();
static_assert(C7<int, long> && !C7<int, char>);

template <class T, class U>
concept C8 = [] { auto g = [] { return U{}; }; return sizeof(g()) > 1; }();
static_assert(C8<int, long> && !C8<int, char>);

// equivalent mappings: the lambda names only T, so the U argument does not
// enter the key and both checks agree
template <class T, class U> concept C9 = [] { return sizeof(T) > 1; }();
static_assert(C9<int, char> && C9<int, long> && !C9<char, int>);

template <class T, class U> concept M = [] { return sizeof(T) > 0; }() && sizeof(U) > 1;
static_assert(M<int, long>);
template <class T> requires M<T, char> void h(T);
static_assert(M<int, long>);
static_assert(M<int, char>);
