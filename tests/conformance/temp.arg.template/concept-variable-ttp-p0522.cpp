// [temp.arg.template]/3-4 (P0522 with P2841R7): a concept or variable
// template argument matches a template template parameter of the same kind
// when the parameter is at least as specialized as the argument -- a
// defaulted or pack parameter of the argument absorbs the parameter's
// arguments; a parameter list the argument cannot accept is diagnosed with
// clang 22's wording (spec/sema-templates.md §2.1.1). WP-746.
//@ expect: compile-fail
//@ strict-diags
//@ requires: clang>=22
//@ diag: 25:26 error "template argument for non-type template parameter must be an expression"
//@ diag: 36:3 note "template template argument has different template parameters than its corresponding template template parameter"
//@ diag: 25:33 note "previous template template parameter is here"
//@ diag: 24:15 note "template parameter is declared here"
//@ diag: 37:3 error "too few template arguments for concept 'Same'"
//@ diag: 37:3 note "template template argument has different template parameters than its corresponding template template parameter"
//@ diag: 26:36 note "previous template template parameter is here"
//@ diag: 19:37 note "template is declared here"
template <class T> concept Int = __is_same(T, int);
template <class T, class U> concept Same = __is_same(T, U);
template <class... Ts> concept AllInt = (__is_same(Ts, int) && ...);
template <class T> constexpr int sz = sizeof(T) == 1 ? 1 : 9;
template <class T, class U = int> constexpr int sz2 = sz<T> + sz<U>;
template <class... Ts> constexpr int cnt = sizeof...(Ts);
template <int N> constexpr int nv = N;
template <template <class> auto V> struct W { static constexpr int v = V<char>; };
template <template <class> concept C> struct X { static constexpr bool b = C<int>; };
template <template <class, class> concept C2> struct X2 { static constexpr bool b = C2<int, int>; };
template <template <class...> concept CP> struct XP { static constexpr bool b = CP<int, int>; };
static_assert(W<sz>::v == 1);
static_assert(W<sz2>::v == 10);
static_assert(W<cnt>::v == 1);
static_assert(X<AllInt>::b);
static_assert(X2<Same>::b);
static_assert(X2<AllInt>::b);
static_assert(XP<AllInt>::b);
W<nv> e1;
X<Same> e2;
