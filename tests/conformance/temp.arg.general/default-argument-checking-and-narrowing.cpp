// [temp.arg.general]/9, [temp.arg.nontype]/2, [expr.const]/12: a default
// template argument is checked as a converted constant expression where the
// template-id uses it ("while checking a default template argument used
// here" at its `>`), and a class-type argument converted by a user-defined
// conversion narrows through the standard conversion after it.
//@ std: c++20
//@ expect: compile-fail
//@ strict-diags
//@ diag: 33:28 error "non-type template argument evaluates to 1099511627776, which cannot be narrowed to type 'int'"
//@ diag: 35:4 note "while checking a default template argument used here"
//@ diag: 33:28 error "non-type template argument evaluates to 1099511627776, which cannot be narrowed to type 'int'"
//@ diag: 37:6 note "while checking a default template argument used here"
//@ diag: 38:26 error "non-type template argument evaluates to 300, which cannot be narrowed to type 'char'"
//@ diag: 39:7 note "while checking a default template argument used here"
//@ diag: 43:26 error "non-type template argument is not a constant expression"
//@ diag: 44:4 note "while checking a default template argument used here"
//@ diag: 42:36 note "division by zero"
//@ diag: 43:26 note "in call to 'g(0)'"
//@ diag: 33:31 error "no member named 'value' in 'W'"
//@ diag: 48:1 note "in instantiation of default argument for 'A<W>' required here"
//@ diag: 51:28 error "non-type template argument evaluates to 1099511627776, which cannot be narrowed to type 'int'"
//@ diag: 52:5 note "while checking a default template argument used here"
//@ diag: 53:28 error "non-type template argument evaluates to 1099511627776, which cannot be narrowed to type 'int'"
//@ diag: 54:13 note "while checking a default template argument used here"
//@ diag: 55:28 error "non-type template argument evaluates to 1099511627776, which cannot be narrowed to type 'int'"
//@ diag: 56:14 note "while checking a default template argument used here"
//@ diag: 62:3 error "non-type template argument evaluates to 2, which cannot be narrowed to type 'bool'"
//@ diag: 63:29 error "non-type template argument evaluates to 2, which cannot be narrowed to type 'bool'"
//@ diag: 64:5 note "while checking a default template argument used here"
//@ diag: 68:3 error "non-type template argument evaluates to 300, which cannot be narrowed to type 'char'"
// A default argument that narrows: the note points at the `>` of the
// template-id that used it, for a non-dependent default as well.
template <class T, int N = T::value> struct A {};
struct V { static constexpr long value = 1L << 40; };
A<V> a1;
template <class T> struct B {};
B<A<V> > b1;
template <class T, T N = 300> struct C {};
C<char> c1;

// A default that is not a constant expression.
constexpr int g(int x) { return 10 / x; }
template <int M, int N = g(M)> struct F {};
F<0> f1;

// Substitution failure stays an instantiation of the default argument.
struct W {};
A<W> a2;

// Alias, variable and concept templates.
template <class T, int N = T::value> using AL = int;
AL<V> al;
template <class T, int N = T::value> constexpr int vt = 0;
int x = vt<V>;
template <class T, int N = T::value> concept Cn = true;
bool c = Cn<V>;

// After a user-defined conversion the standard conversion from its result
// narrows.
struct CI { constexpr operator int() const { return 2; } };
template <bool B> struct Z {};
Z<CI{}> z1;
template <class T, bool B = T{}> struct D {};
D<CI> d1;
enum E { e300 = 300 };
struct CE { constexpr operator E() const { return e300; } };
template <char Ch> struct Y {};
Y<CE{}> y1;
Y<CI{}> y2;
