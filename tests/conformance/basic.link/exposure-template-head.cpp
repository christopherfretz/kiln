// [basic.link]/14, /15, /17 (WP-1855): a template-declaration contains its
// template-head, which /15 never ignores, so in a module interface a
// declaration that is not TU-local is an exposure when its head names a
// TU-local entity: a type-constraint's concept-name (also an abbreviated
// function template's `Hid auto` parameter, a template template
// parameter's head, a return-type-requirement), a requires-clause naming a
// TU-local concept, function, type or variable (also in a
// requires-expression, a trailing requires-clause), a template
// parameter's type or default argument. So are a function, class,
// variable and alias template, a member template, a partial
// specialization, a deduction guide, a concept; and the closure type of a
// lambda whose template head names one, when it is the type of the variable it
// initializes or is in an inline function (the error at the lambda), as
// the lambda's body is (WP-1684); a placeholder's type-constraint in a
// variable's type. Exported or not. GCC 16.1 rejects each declaration
// here but `placeholder` (it names the closure type for `default_lambda`);
// clang 22 accepts all of it.
//@ expect: compile-fail
//@ std: c++20
//@ skip(clang): clang 22 does not check template heads
//@ strict-diags
//@ diag: 64:30 error "declaration of 'f1' exposes TU-local entity 'Hid'"
//@ diag: 65:48 error "declaration of 'f2' exposes TU-local entity 'Hid'"
//@ diag: 66:32 error "declaration of 'f3' exposes TU-local entity 'Hid'"
//@ diag: 67:52 error "declaration of 'f4' exposes TU-local entity 'helper'"
//@ diag: 68:57 error "declaration of 'f5' exposes TU-local entity 'S'"
//@ diag: 69:62 error "declaration of 'f6' exposes TU-local entity 'hidden'"
//@ diag: 70:64 error "declaration of 'f7' exposes TU-local entity 'helper'"
//@ diag: 71:13 error "declaration of 'f8' exposes TU-local entity 'Hid'"
//@ diag: 72:48 error "declaration of 'f9' exposes TU-local entity 'Hid'"
//@ diag: 73:32 error "declaration of 'C1' exposes TU-local entity 'Hid'"
//@ diag: 74:50 error "declaration of 'C2' exposes TU-local entity 'Hid'"
//@ diag: 75:49 error "declaration of 'A1' exposes TU-local entity 'Hid'"
//@ diag: 76:47 error "declaration of 'v1' exposes TU-local entity 'Hid'"
//@ diag: 77:41 error "declaration of 'not_exported' exposes TU-local entity 'Hid'"
//@ diag: 78:64 error "declaration of 'm' exposes TU-local entity 'Hid'"
//@ diag: 78:112 error "declaration of 'In' exposes TU-local entity 'Hid'"
//@ diag: 79:72 error "declaration of 'In' exposes TU-local entity 'Hid'"
//@ diag: 80:36 error "declaration of 'd1' exposes TU-local entity 'S'"
//@ diag: 81:29 error "declaration of 'd2' exposes TU-local entity 'S'"
//@ diag: 82:42 error "declaration of 'd3' exposes TU-local entity 'S'"
//@ diag: 83:53 error "declaration of 'd4' exposes TU-local entity 'S'"
//@ diag: 84:62 error "declaration of 'default_lambda' exposes TU-local entity 'Hid'"
//@ diag: 86:25 error "declaration of 'P' exposes TU-local entity 'Hid'"
//@ diag: 88:22 error "declaration of 'pv' exposes TU-local entity 'Hid'"
//@ diag: 90:18 error "declaration of '<deduction guide for G>' exposes TU-local entity 'Hid'"
//@ diag: 91:32 error "declaration of 'trailing_req' exposes TU-local entity 'Hid'"
//@ diag: 92:32 error "declaration of 'trailing_body' exposes TU-local entity 'helper'"
//@ diag: 93:17 error "declaration of 'placeholder' exposes TU-local entity 'Hid'"
//@ diag: 94:19 error "declaration of 'lam' exposes TU-local entity 'Hid'"
//@ diag: 95:33 error "declaration of 'lam_inline' exposes TU-local entity 'Hid'"
//@ diag: 96:24 error "declaration of 'lam_auto' exposes TU-local entity 'Hid'"
//@ diag: 97:25 error "declaration of 'lam_param' exposes TU-local entity 'S'"
//@ diag: 98:41 error "declaration of 'in_body' exposes TU-local entity 'Hid'"
//@ diag: 99:39 error "declaration of 'C5' exposes TU-local entity 'S'"
//@ diag: 100:45 error "declaration of 'C6' exposes TU-local entity 'S'"
//@ args: -ferror-limit=0
export module M;
static bool helper() { return true; }
static int hidden;
namespace { template <class T> concept Hid = true; struct S {}; }
template <class T> concept Pub = true;
template <class T> struct Box {};
export template <Hid T> void f1(T);
export template <class T> requires Hid<T> void f2(T);
export template <class T> void f3(T) requires Hid<T>;
export template <class T> requires (helper()) void f4(T);
export template <class T> requires (sizeof(S) > 0) void f5(T);
export template <class T> requires (sizeof(hidden) > 0) void f6(T);
export template <class T> requires requires { helper(); } void f7(T);
export void f8(Hid auto x);
export template <template <Hid> class TT> void f9();
export template <Hid T> struct C1 {};
export template <class T> requires Hid<T> struct C2 {};
export template <class T> requires Hid<T> using A1 = T;
export template <class T> requires Hid<T> int v1 = 0;
template <class T> requires Hid<T> void not_exported(T);
export template <class T> struct Outer { template <Hid U> void m(U); template <class U> requires Hid<U> struct In; };
template <class T> template <class U> requires Hid<U> struct Outer<T>::In {};
export template <class T = S> void d1();
export template <S *p> void d2();
export template <int N = sizeof(S)> void d3();
export template <class T> requires Pub<Box<S>> void d4();
export template <int N = []<Hid T>(T) { return 0; }(1)> void default_lambda();
export template <class T> struct P {};
template <Hid T> struct P<T *> {};
export template <class T> int pv = 0;
template <Hid T> int pv<T *> = 1;
export template <class T> struct G { G(T); };
template <Hid T> G(T) -> G<T>;
export template <class T> void trailing_req() requires requires(T t) { { t } -> Hid; };
export template <class T> void trailing_body(T) requires requires { helper(); };
export Hid auto placeholder = 1;
export auto lam = []<Hid T>(T) {};
export inline auto lam_inline = []<Hid T>(T) {};
export auto lam_auto = [](Hid auto) {};
export auto lam_param = [](S) {};
export inline void in_body() { auto l = []<Hid T>(T) {}; l(1); }
export template <class T = S> concept C5 = true;
export template <int N = sizeof(S)> concept C6 = true;
