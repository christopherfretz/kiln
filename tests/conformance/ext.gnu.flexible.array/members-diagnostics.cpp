// Flexible array members in C++ (spec/sema.md §5.8; a C99 feature and GNU
// extension, clang `Sema::ActOnFields`): accepted as the last member of a
// struct or class and in a union, with clang's pedantic extension warnings
// and its errors for a member not at the end, a class with a virtual base,
// and an element type with non-trivial destruction.
//@ expect: compile-fail
//@ strict-diags
//@ args: -pedantic
//@ diag: 41:24 warning "flexible array members are a C99 feature"
//@ diag: 42:17 warning "flexible array member 'd' in otherwise empty struct is a GNU extension"
//@ diag: 42:17 warning "flexible array members are a C99 feature"
//@ diag: 43:17 error "flexible array member 'd' with type 'char[]' is not at the end of struct"
//@ diag: 43:26 note "next field declaration is here"
//@ diag: 45:31 warning "flexible array members are a C99 feature"
//@ diag: 46:40 error "flexible array member 'd' not allowed in struct which has a virtual base class"
//@ diag: 46:40 warning "flexible array members are a C99 feature"
//@ diag: 47:23 warning "flexible array member 'd' in a union is a GNU extension"
//@ diag: 47:23 warning "flexible array members are a C99 feature"
//@ diag: 48:17 warning "flexible array member 'd' in a union is a GNU extension"
//@ diag: 48:17 warning "flexible array members are a C99 feature"
//@ diag: 50:22 warning "flexible array members are a C99 feature"
//@ diag: 50:22 error "flexible array member 'd' of type 'NT[]' with non-trivial destruction"
//@ diag: 52:22 warning "flexible array members are a C99 feature"
//@ diag: 53:14 warning "field 'a' with variable sized type 'A' not at the end of a struct or class is a GNU extension"
//@ diag: 54:24 warning "'a' may not be nested in a struct due to flexible array member"
//@ diag: 55:22 warning "'A' may not be used as an array element due to flexible array member"
//@ diag: 55:21 warning "flexible array members are a C99 feature"
//@ diag: 56:40 warning "flexible array members are a C99 feature"
//@ diag: 57:26 warning "zero size arrays are an extension"
//@ diag: 58:40 warning "flexible array members are a C99 feature"
//@ diag: 58:40 warning "flexible array members are a C99 feature"
//@ diag: 59:8 note "in instantiation of template class 'X<int>' requested here"
//@ diag: 58:40 warning "flexible array members are a C99 feature"
//@ diag: 60:7 note "in instantiation of template class 'X<NT>' requested here"
//@ diag: 58:40 error "flexible array member 'd' of type 'NT[]' with non-trivial destruction"
//@ diag: 61:27 error "array bound cannot be deduced from a default member initializer"
//@ diag: 63:23 error "field has incomplete type 'Inc'"
//@ diag: 62:8 note "forward declaration of 'Inc'"
//@ diag: 65:24 error "array of abstract class type 'Abs'"
//@ diag: 64:27 note "unimplemented pure virtual method 'f' in 'Abs'"
struct A { int n; char d[]; };
struct B { char d[]; };
struct C { char d[]; int n; };
struct Base { int x; };
struct D : Base { int n; char d[]; };
struct VB : virtual Base { int n; char d[]; };
union U { int n; char d[]; };
union U2 { char d[]; };
struct NT { NT(); ~NT(); };
struct F { int n; NT d[]; };
struct NC { NC(); };
struct G { int n; NC d[]; };
struct W { A a; int x; };
struct Last { int x; A a; };
struct H { int n; A a[]; };
class P { public: int n; private: char d[]; };
struct Z { int n; char d[0]; };
template <class T> struct X { int n; T d[]; };
X<int> xi;
X<NT> xn;
struct M { int n; int d[] = {}; };
struct Inc;
struct I { int n; Inc d[]; };
struct Abs { virtual void f() = 0; };
struct J { int n; Abs d[]; };
