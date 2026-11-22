// [dcl.inline]/6, [class.friend]: "attribute declaration must precede
// definition" (clang `checkNewAttributesAfterDef`, -Wignored-attributes)
// on a friend redeclaration after the function's definition, also when
// the friend is invalid: an `inline` friend after a non-inline
// definition, in a class and in a class template (checked at the
// template's definition, as clang does), unqualified and qualified; and
// a namespace-scope redeclaration after a friend definition. spec/sema.md
// §4.5, WP-1850. Verified against clang 22 and clang 19.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 31:35 warning "attribute declaration must precede definition"
//@ diag: 30:6 note "previous definition is here"
//@ diag: 31:54 error "inline declaration of 'g1' follows non-inline definition"
//@ diag: 30:6 note "previous definition is here"
//@ diag: 33:54 warning "attribute declaration must precede definition"
//@ diag: 32:6 note "previous definition is here"
//@ diag: 33:73 error "inline declaration of 'g2' follows non-inline definition"
//@ diag: 32:6 note "previous definition is here"
//@ diag: 36:16 warning "attribute declaration must precede definition"
//@ diag: 35:25 note "previous definition is here"
//@ diag: 38:35 warning "attribute declaration must precede definition"
//@ diag: 37:6 note "previous definition is here"
//@ diag: 38:56 error "inline declaration of 'g4' follows non-inline definition"
//@ diag: 37:6 note "previous definition is here"
//@ diag: 40:35 warning "attribute declaration must precede definition"
//@ diag: 39:20 note "previous definition is here"
//@ diag: 40:57 error "inline declaration of 'g5' follows non-inline definition"
//@ diag: 39:20 note "previous definition is here"

void g1() {}
struct S1 { friend __attribute__((cold)) inline void g1(); };
void g2() {}
template <class T> struct S2 { friend __attribute__((cold)) inline void g2(); };
S2<int> s2;
struct S3 { friend void g3() {} };
__attribute__((cold)) inline void g3();
void g4() {}
struct S4 { friend __attribute__((cold)) inline void ::g4(); };
namespace N { void g5() {} }
struct S5 { friend __attribute__((cold)) inline void N::g5(); };
