// [temp.arg.nontype], [dcl.enum]: an enumeration template argument prints
// as clang prints it: a written argument as written, its qualifier
// included (`E<order::B>`, `G<ns::e2::X>`, `H<ns::U1>`, `H<U1>` after a
// using-directive, `H<S1>` after a using-enum-declaration, `H<k>`), a
// value with no enumerator as a cast (`H<(order)7>`), and a substituted
// value in a static_assert requirement as a cast (`(order)1`) (WP-186).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 37:13 error "call to deleted constructor of 'E<order::B>'"
//@ diag: 33:30 note "'E' has been explicitly marked deleted here"
//@ diag: 38:7 error "call to deleted constructor of 'F<P1>'"
//@ diag: 34:30 note "'F' has been explicitly marked deleted here"
//@ diag: 39:14 error "call to deleted constructor of 'G<ns::e2::X>'"
//@ diag: 35:31 note "'G' has been explicitly marked deleted here"
//@ diag: 40:13 error "call to deleted constructor of 'H<order::A>'"
//@ diag: 36:29 note "'H' has been explicitly marked deleted here"
//@ diag: 41:13 error "call to deleted constructor of 'H<(order)7>'"
//@ diag: 36:29 note "'H' has been explicitly marked deleted here"
//@ diag: 42:44 error "static assertion failed due to requirement '(order)1 == order::A'"
//@ diag: 43:13 note "in instantiation of template class 'I<order::B>' requested here"
//@ diag: 42:46 note "expression evaluates to '1 == 0'"
//@ diag: 45:7 error "call to deleted constructor of 'H<U1>'"
//@ diag: 36:29 note "'H' has been explicitly marked deleted here"
//@ diag: 46:11 error "call to deleted constructor of 'H<ns::U1>'"
//@ diag: 36:29 note "'H' has been explicitly marked deleted here"
//@ diag: 48:7 error "call to deleted constructor of 'H<S1>'"
//@ diag: 36:29 note "'H' has been explicitly marked deleted here"
//@ diag: 50:6 error "call to deleted constructor of 'H<k>'"
//@ diag: 36:29 note "'H' has been explicitly marked deleted here"
enum class order { A, B };
enum plain { P0, P1 };
namespace ns { enum class e2 { X = 5 }; enum u { U1 }; enum class s { S1 }; }
template<order O> struct E { E() = delete; };
template<plain P> struct F { F() = delete; };
template<ns::e2 Q> struct G { G() = delete; };
template<auto V> struct H { H() = delete; };
E<order::B> e;
F<P1> f;
G<ns::e2::X> g;
H<order::A> h;
H<(order)7> h2;
template<order O> struct I { static_assert(O == order::A); };
I<order::B> i;
using namespace ns;
H<U1> a;
H<ns::U1> a2;
using enum ns::s;
H<S1> b;
constexpr auto k = ns::s::S1;
H<k> d;
