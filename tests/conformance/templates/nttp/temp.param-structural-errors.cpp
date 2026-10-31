// [temp.param]/7: a non-type template parameter of class type must be a
// structural type; clang's notes name the first offending member or base.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 30:13 error "type 'B' of non-type template parameter is not a structural type"
//@ diag: 21:25 note "'B' is not a structural type because it has a non-static data member that is not public"
//@ diag: 31:13 error "type 'C' of non-type template parameter is not a structural type"
//@ diag: 22:24 note "'C' is not a structural type because it has a mutable non-static data member"
//@ diag: 32:13 error "type 'F' of non-type template parameter is not a structural type"
//@ diag: 23:11 note "'F' is not a structural type because it has a base class that is not public"
//@ diag: 33:13 error "type 'G' of non-type template parameter is not a structural type"
//@ diag: 24:14 note "'G' is not a structural type because it has a non-static data member of non-structural type 'B'"
//@ diag: 21:25 note "'B' is not a structural type because it has a non-static data member that is not public"
//@ diag: 35:13 error "type 'I' of non-type template parameter is not a structural type"
//@ diag: 26:18 note "'I' is not a structural type because it has a non-static data member of rvalue reference type"
//@ diag: 37:13 error "type 'K' of non-type template parameter is not a structural type"
//@ diag: 28:14 note "'K' is not a structural type because it has a non-static data member of non-structural type 'C'"
//@ diag: 22:24 note "'C' is not a structural type because it has a mutable non-static data member"
//@ diag: 38:17 error "non-type template parameter has rvalue reference type 'int &&'"
struct A { int x; };
struct B { private: int x; };
struct C { mutable int x; };
class F : A {};
struct G { B b; };
struct H { int& r; };
struct I { int&& r; };
struct J { float f; double d[2]; };
struct K { C c[2]; };
template <A a> struct TA {};
template <B b> struct TB {};
template <C c> struct TC {};
template <F f> struct TF {};
template <G g> struct TG {};
template <H h> struct TH {};
template <I i> struct TI {};
template <J j> struct TJ {};
template <K k> struct TK {};
template <int&& r> struct TR {};
