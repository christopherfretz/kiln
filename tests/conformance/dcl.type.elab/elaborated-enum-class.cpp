// [dcl.type.elab]/1: an elaborated-type-specifier for an enumeration uses
// plain 'enum', never 'enum class' or 'enum struct' (clang
// -Welaborated-enum-class, an error by default); an
// opaque-enum-declaration keeps its enum-key. WP-1017.
//@ std: c++20
//@ expect: compile-fail
//@ strict-diags
//@ diag: 13:6 error "reference to enumeration must use 'enum' not 'enum class'"
//@ diag: 14:6 error "reference to enumeration must use 'enum' not 'enum struct'"
//@ diag: 18:13 error "reference to enumeration must use 'enum' not 'enum class'"
enum class E;
enum struct F : int;
enum class E *p;
enum struct F *q;
enum E *ok1;
enum F *ok2;
enum class G : long;
void f(enum class E e);
void g(enum E e);
