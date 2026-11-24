// [temp.deduct.guide]/3: a deduction guide inhabits the scope to which its
// class template belongs. A guide whose template is found through a
// using-directive, a using-declaration, the enclosing namespace of an
// inline namespace or a base class is in another scope: rejected at the
// guide's name, the template noted, the guide invalid (no follow-on
// diagnostics). A linkage specification is no scope of its own. Reduced
// from real code.
//@ expect: compile-fail
//@ strict-diags
//@ args: -std=c++20
//@ diag: 29:19 error "deduction guide must be declared in the same scope as template 'n::A'"
//@ diag: 27:40 note "template is declared here"
//@ diag: 30:10 error "deduction guide must be declared in the same scope as template 'n::A'"
//@ diag: 27:40 note "template is declared here"
//@ diag: 31:1 error "deduction guide must be declared in the same scope as template 'n::A'"
//@ diag: 27:40 note "template is declared here"
//@ diag: 32:34 error "deduction guide must be declared in the same scope as template 'n::A'"
//@ diag: 27:40 note "template is declared here"
//@ diag: 33:45 error "deduction guide must be declared in the same scope as template 'n::A'"
//@ diag: 27:40 note "template is declared here"
//@ diag: 35:33 error "deduction guide must be declared in the same scope as template 'o::B'"
//@ diag: 34:61 note "template is declared here"
//@ diag: 37:35 error "deduction guide must be declared in the same scope as template 'S::M'"
//@ diag: 36:37 note "template is declared here"
//@ diag: 38:52 error "deduction guide must be declared in the same scope as template 'S::M'"
//@ diag: 36:37 note "template is declared here"
namespace n { template<class T> struct A { A(T); }; }
using namespace n;
template<class T> A(T) -> A<T>;
explicit A(char) -> A<char>;
A(int) -> A<int>;
extern "C++" { template<class T> A(T*) -> A<T>; }
namespace m { using n::A; template<class T> A(T**) -> A<T>; }
namespace o { inline namespace v { template<class T> struct B { B(T); }; } }
namespace o { template<class T> B(T) -> B<T>; }
struct S { template<class T> struct M { M(T); }; template<class T> M(T) -> M<T>; };
struct S2 : S { template<class T> M(T*) -> M<T>; };
template<class U> struct D : S { template<class T> M(T&) -> M<T>; };
// Valid: the template's own scope, reopened or through a linkage
// specification.
namespace n { template<class T> A(T&) -> A<T>; }
namespace n { extern "C++" { template<class T> A(const T*) -> A<T>; } }
namespace o { inline namespace v { template<class T> B(T*) -> B<T>; } }
