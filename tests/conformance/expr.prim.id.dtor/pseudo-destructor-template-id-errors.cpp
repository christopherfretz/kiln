// [expr.prim.id.dtor]/2, [basic.lookup.qual.general]/4, [temp.names]/3:
// in a pseudo-destructor-name `p->[S::]F::~Q` the template-name of a
// `~Q<args>` is looked up in the prefix `S` (4.2), or where the expression
// is (4.3); it must name a type template, and both type-names must name
// the object's type. A bare alias-template name `~A` is not a type-name
// (an alias template has no injected-class-name), also at instantiation;
// a namespace is not a type-name `F` (4.2: N shall nominate a type); on
// a dependent object a name found nowhere is looked up at instantiation.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 35:34 error "use of undeclared 'Nope'"
//@ diag: 36:39 error "'v' does not refer to a template"
//@ diag: 31:5 note "declared as a non-template here"
//@ diag: 37:33 error "no template named 'A' in namespace 'N'; did you mean simply 'A'?"
//@ diag: 27:20 note "'A' declared here"
//@ diag: 38:32 error "the type of object expression ('int') does not match the type being destroyed ('A<long>' (aka 'long')) in pseudo-destructor expression"
//@ diag: 39:39 error "'A' does not refer to a type name in pseudo-destructor expression; expected the name of type 'int'"
//@ diag: 40:52 error "'B' does not refer to a type name in pseudo-destructor expression; expected the name of type 'int'"
//@ diag: 41:43 error "use of undeclared 'Nope'"
//@ diag: 42:36 error "'N' does not refer to a type name in pseudo-destructor expression; expected the name of type 'int'"
//@ diag: 43:41 error "no member named '~T' in namespace 'N'"
//@ diag: 45:73 error "'A' does not refer to a type name in pseudo-destructor expression; expected the name of type 'int'"
//@ diag: 44:64 error "undeclared identifier 'A' in destructor name"
//@ diag: 48:32 note "in instantiation of function template specialization 'dependent_alias_name<int>' requested here"
//@ diag: 47:64 error "undeclared identifier 'D' in destructor name"
//@ diag: 48:57 note "in instantiation of function template specialization 'dependent_unknown<int>' requested here"
template <class T> using A = T;
namespace N {
using I = int;
template <class T> using B = T;
int v;
struct T { ~T(); };
}
using I = int;
void undeclared(int* p) { p->I::~Nope<int>(); }
void non_template(int* p) { p->N::I::~v<int>(); }
void not_in_prefix(int* p) { p->N::I::~A<int>(); }
void mismatch(int* p) { p->I::~A<long>(); }
void alias_name(int* p) { p->A<int>::~A(); }
void alias_name_in_prefix(int* p) { p->N::B<int>::~B(); }
void unqualified_undeclared(int* p) { p->~Nope<int>(); }
void namespace_scalar(int* p) { p->N::~I(); }
void namespace_class(N::T* p) { p->::N::~T(); }
template <class X> void dependent_alias_name(X* p) { p->A<X>::~A(); }
template <class X> void scalar_template_kw(int* p) { p->template A<X>::~A(); }
template <class X> struct C { ~C(); };
template <class X> void dependent_unknown(C<X>* p) { p->C<X>::~D(); }
void inst(int* p, C<int>* c) { dependent_alias_name(p); dependent_unknown(c); }
