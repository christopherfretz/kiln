// [temp.arg.nontype]/6: for a pointer or reference template parameter,
// and for each pointer or reference subobject of a class-type template
// argument (at any depth: members, nested members, base class
// subobjects, array elements, a union's active member), the value must
// not point to a string literal object, a predefined `__func__`
// variable, a temporary object or a `typeid` result, or a subobject of
// one (clang `CheckEvaluationResult` -> `CheckLValueConstantExpression`,
// `err_constexpr_invalid_template_arg`; one error per argument, at the
// argument). A pointer into or one past a string literal is a pointer
// to a subobject; a reference to the whole literal is not.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 36:3 error "pointer to subobject of string literal is not allowed in a template argument"
//@ diag: 37:3 error "pointer to subobject of string literal is not allowed in a template argument"
//@ diag: 38:3 error "pointer to subobject of string literal is not allowed in a template argument"
//@ diag: 39:3 error "pointer to subobject of string literal is not allowed in a template argument"
//@ diag: 40:3 error "pointer to subobject of string literal is not allowed in a template argument"
//@ diag: 41:3 error "pointer to subobject of string literal is not allowed in a template argument"
//@ diag: 42:3 error "reference to string literal is not allowed in a template argument"
//@ diag: 43:3 error "pointer to subobject of string literal is not allowed in a template argument"
//@ diag: 44:3 error "pointer to subobject of string literal is not allowed in a template argument"
//@ diag: 45:3 error "pointer to subobject of string literal is not allowed in a template argument"
//@ diag: 46:3 error "pointer to type_info object is not allowed in a template argument"
//@ diag: 47:40 error "pointer to subobject of predefined '__func__' variable is not allowed in a template argument"
#include <typeinfo>
struct T { const char* p; };
struct N { T t; int i; };
struct B : T { int j; };
struct A { const char* a[2]; };
struct R { const char (&r)[2]; };
struct AR { T ts[2]; };
union U { int i; const char* p; };
struct TI { const std::type_info* p; };
template<auto V> struct X {};
constexpr const char* lit = "abc";
X<T{"x"}> x1;
X<N{{"ab"}, 1}> x2;
X<B{{"ab"}, 1}> x3;
X<A{{nullptr, "q"}}> x4;
X<T{lit + 1}> x5;
X<T{lit + 4}> x6;
X<R{"x"}> x7;
X<AR{{{nullptr}, {"z"}}}> x8;
X<U{.p = "u"}> x9;
X<T{__builtin_FUNCTION()}> x10;
X<TI{&typeid(int)}> x11;
void fn() { constexpr T t{__func__}; X<t> x; }
