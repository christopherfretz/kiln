// [dcl.type.elab]/3: the class-key or enum keyword of an elaborated-type-
// specifier agrees with the declaration it refers to (struct and class are
// interchangeable; union and enum are not). Covers plain and qualified
// names, explicit and partial specializations whose class-key differs from
// the primary's, and references to specializations (clang
// err_use_with_wrong_tag at the keyword, 'previous use is here'). WP-1017.
//@ std: c++20
//@ expect: compile-fail
//@ strict-diags
//@ diag: 27:1 error "use of 'S' with tag type that does not match previous declaration"
//@ diag: 26:8 note "previous use is here"
//@ diag: 32:1 error "use of 'Member' with tag type that does not match previous declaration"
//@ diag: 30:37 note "previous use is here"
//@ diag: 34:13 error "use of 'Y' with tag type that does not match previous declaration"
//@ diag: 33:27 note "previous use is here"
//@ diag: 35:20 error "use of 'Y' with tag type that does not match previous declaration"
//@ diag: 33:27 note "previous use is here"
//@ diag: 38:1 error "Y<int>' with tag type that does not match previous declaration"
//@ diag: 34:19 note "previous use is here"
//@ diag: 40:1 error "Y<char>' with tag type that does not match previous declaration"
//@ diag: 33:27 note "previous use is here"
//@ diag: 42:1 error "use of 'E4' with tag type that does not match previous declaration"
//@ diag: 41:17 note "previous use is here"
//@ diag: 46:17 error "no struct named 'K' in 'Z<long>'"
//@ diag: 47:16 error "no union named 'K' in 'Z<long>'"
struct S {};
union S *p;
struct S *ok1;
class S *ok2;
template <class T> struct X { union Member {}; };
union X<int>::Member *ok3;
struct X<int>::Member *bad;
template <class T> struct Y {};
template <> union Y<int> {};
template <class T> union Y<T *> {};
template <> class Y<long> {};
struct Y<int> *bad2;
union Y<int> *ok4;
class Y<char> *ok5;
union Y<char> *bad3;
struct N { enum E4 {}; };
struct N::E4 *bad4;
enum N::E4 *ok6;
template <class T> struct Z {};
template <> struct Z<int> { struct K {}; };
struct Z<long>::K *bad5;
union Z<long>::K *bad6;
