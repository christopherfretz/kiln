// [over.literal]/2, [namespace.udecl]/1: a literal operator is a namespace
// member, so a using-declarator naming one through a class or dependent
// qualifier is ill-formed at namespace, class and block scope, also in a
// template definition. WP-559.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 12:10 error "cannot have a literal operator member"
//@ diag: 13:25 error "cannot have a literal operator member"
//@ diag: 14:25 error "cannot have a literal operator member"
//@ diag: 15:43 error "cannot have a literal operator member"
struct U {};
using U::operator""_y;
struct D : U { using U::operator""_y; };
void block() { using U::operator""_y; }
template <class T> void tmpl() { using T::operator""_x; }
