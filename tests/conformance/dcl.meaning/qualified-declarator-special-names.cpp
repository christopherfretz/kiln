// [dcl.meaning.general]/2, [temp.expl.spec]/3: a qualified declarator-id
// must be declared in a scope enclosing its qualifier, whatever its form:
// an operator-function-id, conversion-function-id, literal-operator-id,
// constructor or destructor name as well as an identifier (clang's
// diagnoseQualifiedDeclaration; the scope is printed unqualified), with
// -Wextra-qualification for a redundant namespace qualifier. A function
// template explicit specialization outside a namespace enclosing the
// template is clang's CheckTemplateSpecializationScope error. A leading
// `::` anywhere in the qualifier (`G (::N::x)`) suppresses
// -Wredundant-parens; `G (N::x)` warns. WP-235.
//@ expect: compile-fail
//@ args: -Wredundant-parens -Wextra-qualification
//@ strict-diags
// Verified against clang 19 (WP-235).
//@ diag: 47:14 error "definition or redeclaration of 'operator+' not allowed inside a function"
//@ diag: 48:11 error "definition or redeclaration of 'operator*' not allowed inside a function"
//@ diag: 49:11 error "definition or redeclaration of 'operator\"\"_y' not allowed inside a function"
//@ diag: 52:9 error "cannot define or redeclare 'operator*' here because namespace 'M' does not enclose namespace 'N'"
//@ diag: 53:12 error "cannot define or redeclare 'operator-' here because namespace 'M' does not enclose namespace 'C'"
//@ diag: 54:7 error "cannot define or redeclare 'operator long' here because namespace 'M' does not enclose namespace 'C'"
//@ diag: 55:7 error "cannot define or redeclare '~C' here because namespace 'M' does not enclose namespace 'C'"
//@ diag: 56:7 error "cannot define or redeclare 'C' here because namespace 'M' does not enclose namespace 'C'"
//@ diag: 57:9 error "cannot define or redeclare 'operator\"\"_y' here because namespace 'M' does not enclose namespace 'N'"
//@ diag: 58:15 error "cannot define or redeclare 'e' here because namespace 'M' does not enclose namespace 'E'"
//@ diag: 59:20 error "function template specialization of 'h' not in a namespace enclosing 'N'"
//@ diag: 42:24 note "explicitly specialized declaration is here"
//@ diag: 62:11 error "non-friend class member 'operator*' cannot have a qualified name"
//@ diag: 63:9 error "non-friend class member '~B' cannot have a qualified name"
//@ diag: 64:14 error "non-friend class member 'operator+' cannot have a qualified name"
//@ diag: 67:9 warning "extra qualification on member 'operator*'"
//@ diag: 68:9 warning "extra qualification on member 'operator\"\"_y'"
//@ diag: 73:8 error "definition or redeclaration of 'y' cannot name the global scope"
//@ diag: 74:11 error "definition or redeclaration of 'x' not allowed inside a function"
//@ diag: 75:9 error "definition or redeclaration of 'x' not allowed inside a function"
//@ diag: 75:5 warning "redundant parentheses surrounding declarator"
struct A { struct B { void operator+(int); operator int(); ~B(); B(); }; };
struct S {};
namespace N {
struct C { void operator-(int); operator long(); ~C(); C(); };
void operator*(S, int);
void operator""_y(const char*);
template<class T> void h(T);
int x;
}
namespace P { namespace Q { struct E { void e(); }; } }
void f() {
  void A::B::operator+(int);
  void N::operator*(S, int);
  void N::operator""_y(const char*);
}
namespace M {
void N::operator*(S, int);
void N::C::operator-(int);
N::C::operator long();
N::C::~C();
N::C::C();
void N::operator""_y(const char*);
void P::Q::E::e();
template<> void N::h<int>(int);
}
struct D {
  void N::operator*(S, int);
  A::B::~B();
  void A::B::operator+(int);
};
namespace N {
void N::operator*(S, int);
void N::operator""_y(const char*);
}
struct G { G(int); };
int y;
void g() {
  G (::y);
  G (::N::x);
  G (N::x);
}
