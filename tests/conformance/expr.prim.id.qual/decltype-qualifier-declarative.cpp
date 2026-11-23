// [expr.prim.id.qual]/3: a declarative nested-name-specifier shall not have a
// computed-type-specifier. A decltype qualifier on a variable or member
// function definition is an error at the name; a member redeclaration in a
// class or block reports its own rule. A friend declarator may use one (line
// 7 is valid; see decltype-qualifier-friend-declarator-run.cpp).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 14:18 error "a 'decltype' specifier cannot be used in a declarative nested name specifier"
//@ diag: 15:19 error "a 'decltype' specifier cannot be used in a declarative nested name specifier"
//@ diag: 16:30 error "non-friend class member 'k' cannot have a qualified name"
//@ diag: 17:30 error "definition or redeclaration of 'k' not allowed inside a function"
struct A { void g(); static int x; void k(); int m(int); };
A a;
int decltype(a)::x = 1;
void decltype(a)::g() {}
struct Q { void decltype(a)::k(); };
void h() { void decltype(a)::k(); }
template <class T> struct S { friend int decltype(T())::m(int); };
S<A> s;
