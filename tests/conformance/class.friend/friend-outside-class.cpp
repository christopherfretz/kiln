// [class.friend]/1: a friend declaration appears in a class. Elsewhere
// (a parameter, namespace or block scope) `friend` is "'friend' used
// outside of class", reported while parsing the decl-specifiers and
// dropped, so the declaration is otherwise as if it were absent.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 10:8 error "'friend' used outside of class"
//@ diag: 11:1 error "'friend' used outside of class"
//@ diag: 13:12 error "'friend' used outside of class"
void f(friend int x);
friend int y;
int z = y;
void g() { friend int loc = 1; (void)loc; }
struct S { friend class C; };
class C {};
