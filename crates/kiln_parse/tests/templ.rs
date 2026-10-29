//! Per-rule tests for template parsing (`spec/sema-templates.md` §2): the
//! template-parameter kind decision (§2.1) and the `<` rule (§2.2).

mod common;

use common::parse;

/// §2.1: each rule of the kind decision, one parameter per rule, in a list
/// whose defaulted parameters come last so no ordering rule fires.
#[test]
fn param_kind_decision() {
    let p = parse(
        "template <class T> concept C = true;\n\
         struct S { using type = int; };\n\
         struct Q;\n\
         template <typename T, class... Ts, typename S::type N, struct Q *q, C U,\n\
                   C auto V, auto W, template <class> class TT, typename = void>\n\
         void k();\n",
    );
    p.assert_clean();
    // Rule 1: `typename`/`class` followed by an identifier, `...`, `=`, `,`
    // or `>` introduces a type parameter.
    p.assert_dump("TemplateTypeParmDecl typename depth 0 index 0 T");
    p.assert_dump("TemplateTypeParmDecl class depth 0 index 1 ... Ts");
    p.assert_dump("TemplateTypeParmDecl typename depth 0 index 8\n");
    // Rule 2: `typename` followed by a nested-name-specifier starts a
    // non-type parameter whose type is a dependent-or-not typename type.
    p.assert_dump("NonTypeTemplateParmDecl 'type':'int' depth 0 index 2 N");
    // Rule 3: an elaborated `struct Q` is a non-type parameter.
    p.assert_dump("NonTypeTemplateParmDecl 'struct Q *' depth 0 index 3 q");
    // Rule 4: a concept name not followed by `auto` is a constrained type
    // parameter; followed by `auto`, a placeholder non-type parameter.
    p.assert_dump("TemplateTypeParmDecl class depth 0 index 4 U");
    p.assert_dump("NonTypeTemplateParmDecl 'C auto' depth 0 index 5 V");
    // Rule 5: anything else is a non-type parameter (here `auto`), and
    // `template <...> class` a template template parameter.
    p.assert_dump("NonTypeTemplateParmDecl 'auto' depth 0 index 6 W");
    p.assert_dump("TemplateTemplateParmDecl depth 0 index 7 TT");
    p.assert_dump("TemplateTypeParmDecl class depth 1 index 0");
}

/// §2.1 (ordering): a pack that is not last and a missing default are
/// diagnosed on a class template but not on a function template.
#[test]
fn param_order_rules() {
    let p = parse("template <class... Ts, class U> struct A;\n");
    p.assert_diag("1:20 error: template parameter pack must be the last template parameter");
    let p = parse("template <class T = int, class U> struct B;\n");
    p.assert_diag("1:32 error: template parameter missing a default argument");
    p.assert_diag("1:21 note: previous default template argument defined here");
    parse("template <class T = int, class U> void f(U);\n").assert_clean();
    parse("template <class... Ts, class U> void g(U, Ts...);\n").assert_clean();
}

/// §2.2: with no template name on the left, `<` is less-than; `x < y > (0)`
/// is two comparisons.
#[test]
fn less_than_without_template_name() {
    let p = parse("int x = 1, y = 2;\nbool b = x < y > (0);\n");
    p.assert_dump(
        "BinaryOperator 'bool' '>'\n    |-ImplicitCastExpr 'int' <IntegralCast>\n    | `-BinaryOperator 'bool' '<'",
    );
    assert!(!p.dump.contains("TemplateArgument"), "{}", p.dump);
}

/// §2.2: a dependent member or qualified name without `template` is not a
/// template name, so `<` after it is less-than.
#[test]
fn less_than_after_dependent_name() {
    let p = parse("template <class T> void m(T t) { (void)(t.h < 1); (void)(T::n < 2); }\n");
    p.assert_clean();
    p.assert_dump("BinaryOperator '<dependent type>' '<'");
    p.assert_dump("CXXDependentScopeMemberExpr '<dependent type>' lvalue .h");
    p.assert_dump("DependentScopeDeclRefExpr '<dependent type>' lvalue");
}

/// §2.2: after a function template name `<` opens a template-argument list,
/// and `>` inside a parenthesized argument does not close it.
#[test]
fn template_args_after_function_template_name() {
    let p = parse(
        "template <class T> int f(T);\n\
         template <int N> int g();\n\
         template <class T, int M> int u(T t) { return f<T>(t) + g<(M > 2)>(); }\n",
    );
    p.assert_clean();
    p.assert_dump("UnresolvedLookupExpr '<dependent type>' lvalue (ADL) = 'f'");
    p.assert_dump("TemplateArgument type 'T'");
    p.assert_dump("TemplateArgument expr");
}

/// §2.2: `>>` closes two nested template-argument lists.
#[test]
fn double_angle_closes_two_lists() {
    let p = parse("template <class T> struct A {};\ntemplate <class T> void h(A<A<T>>);\n");
    p.assert_clean();
    p.assert_dump("ParmVarDecl 'A<A<T>>'");
}

/// §2.2: a dependent member followed by an unambiguous template-argument
/// list needs `template`; with it, the list parses.
#[test]
fn missing_template_keyword() {
    let p = parse("template <class T> void m(T t) { t.g<int>(); }\n");
    p.assert_diag("error: use 'template' keyword to treat 'g' as a dependent template name");
    let p = parse("template <class T> void m(T t) { t.g<1>(); }\n");
    p.assert_diag("error: missing 'template' keyword prior to dependent template name 'g'");
    let p = parse("template <class T> void m(T t) { t.template g<int>(); }\n");
    p.assert_clean();
    p.assert_dump("CXXDependentScopeMemberExpr '<dependent type>' lvalue .g");
}
