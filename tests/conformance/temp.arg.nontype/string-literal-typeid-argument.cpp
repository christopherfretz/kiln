// [temp.arg.nontype]/6: the value of a pointer or reference template
// argument must not point to (a subobject of) a string literal or the
// result of a typeid expression (clang `CheckLValueConstantExpression`
// for a template argument, `err_constexpr_invalid_template_arg`). A
// string literal decays to a pointer to its first element, a subobject;
// `&"gh"` and a reference bind the whole literal. An explicitly-specified
// argument of a function template fails deduction instead.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 23:3 error "pointer to subobject of string literal is not allowed in a template argument"
//@ diag: 24:3 error "pointer to subobject of string literal is not allowed in a template argument"
//@ diag: 25:9 error "no matching function for call to 'f'"
//@ diag: 20:23 note "candidate template ignored: invalid explicitly-specified argument for template parameter 'p'"
//@ diag: 26:3 error "pointer to string literal is not allowed in a template argument"
//@ diag: 27:3 error "pointer to type_info object is not allowed in a template argument"
//@ diag: 29:3 error "reference to string literal is not allowed in a template argument"
#include <typeinfo>
template <const char* p> struct X {};
template <auto p> struct Y {};
template <auto p> int f() { return 1; }
template <const char (*p)[3]> struct Z {};
template <const std::type_info* p> struct T {};
X<"ab"> x;
Y<"cd"> y;
int g = f<"ef">();
Z<&"gh"> z;
T<&typeid(int)> t;
template <const char (&r)[3]> struct R {};
R<"ij"> r;
