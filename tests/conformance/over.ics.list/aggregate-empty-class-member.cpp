// [over.ics.list]/8, [dcl.init.aggr]/4: a braced-init-list converts to
// an aggregate when its elements can initialize the aggregate's
// elements; a member of empty class type (or an array of them) has no
// scalar leaves but still takes an element of its own. libc++'s
// `std::array<T, 0>` holds `__empty __elems_[1]`, so `{.input = {1},
// .expected = {}}` for a struct of such arrays must be viable
// (ranges_remove_copy.pass.cpp; WP-309).
//@ expect: compile-ok
//@ strict-diags
struct E {};
struct A3 { E e; };
struct A0 { E e[1]; };
struct W3 { A3 a; };
struct W0 { A0 a; };
int f(E, int);
int g(A3, int);
int k(A0, int);
int h3(W3, int);
int h0(W0, int);

// The call depends on `i`, so a non-viable call is a substitution
// failure and the requires-expression is false.
template <class T> constexpr bool f0 = requires(T i) { f({}, i); };
template <class T> constexpr bool f1 = requires(T i) { f({{}}, i); };
template <class T> constexpr bool g1 = requires(T i) { g({{}}, i); };
template <class T> constexpr bool g2 = requires(T i) { g({{}, {}}, i); };
template <class T> constexpr bool k1 = requires(T i) { k({{}}, i); };
template <class T> constexpr bool k2 = requires(T i) { k({{}, {}}, i); };
template <class T> constexpr bool h31 = requires(T i) { h3({{}}, i); };
template <class T> constexpr bool h01 = requires(T i) { h0({{}}, i); };
template <class T> constexpr bool h01b = requires(T i) { h0({{{}}}, i); };
template <class T> constexpr bool h02 = requires(T i) { h0({{}, {}}, i); };

static_assert(f0<int>);
static_assert(!f1<int>); // an empty class takes no element
static_assert(g1<int>);
static_assert(!g2<int>);
static_assert(k1<int>);
static_assert(!k2<int>);
static_assert(h31<int>);
static_assert(h01<int>);
static_assert(h01b<int>);
static_assert(!h02<int>);

// The libc++ shape: designated members that are arrays of empty class.
template <int N> struct arr { E elems[1]; };
struct Data { arr<1> input; arr<0> expected; int val; };
constexpr int run(Data) { return 1; }
static_assert(run({.input = {}, .expected = {}, .val = 1}) == 1);
static_assert(run({{}, {}, 1}) == 1);
