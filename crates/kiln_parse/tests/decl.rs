//! Per-rule tests for declarations ([dcl], [basic], [namespace], [class]).

mod common;
use common::parse;

/// [dcl.type.simple]: multi-keyword fundamental types and their order.
#[test]
fn dcl_type_simple_combinations() {
    let p = parse("unsigned long long a; long unsigned int b; signed char c; long double d;");
    p.assert_clean();
    p.assert_dump("VarDecl a 'unsigned long long'");
    p.assert_dump("VarDecl b 'unsigned long'");
    p.assert_dump("VarDecl c 'signed char'");
    p.assert_dump("VarDecl d 'long double'");
}

/// [dcl.type.general]: conflicting specifiers are diagnosed.
#[test]
fn dcl_type_conflicting_specifiers() {
    let p = parse("unsigned float x;");
    p.assert_diag("error:");
}

/// [dcl.ptr]: pointers, cv on the pointer and on the pointee.
#[test]
fn dcl_ptr_cv() {
    let p = parse("const int *const p = nullptr; int **q;");
    p.assert_clean();
    p.assert_dump("VarDecl p 'const int *const'");
    p.assert_dump("VarDecl q 'int **'");
}

/// [dcl.ref]: references must be initialized.
#[test]
fn dcl_ref_requires_init() {
    let p = parse("int &r;");
    p.assert_diag("declaration of reference variable 'r' requires an initializer");
}

/// [dcl.array]: the bound is a converted constant expression.
#[test]
fn dcl_array_bound_folded() {
    let p = parse("constexpr int n = 3; int a[n * 2]; char s[] = \"abc\";");
    p.assert_clean();
    p.assert_dump("VarDecl a 'int[6]'");
    p.assert_dump("VarDecl s 'char[4]'");
}

/// [dcl.array]: a negative bound is ill-formed.
#[test]
fn dcl_array_negative_bound() {
    let p = parse("int a[-1];");
    p.assert_diag("'a' declared as an array with a negative size");
}

/// [dcl.fct]: variadic and parameter-list forms.
#[test]
fn dcl_fct_variadic() {
    let p = parse("int printf(const char *, ...); void v(void);");
    p.assert_clean();
    p.assert_dump("FunctionDecl printf 'int (const char *, ...)'");
    p.assert_dump("FunctionDecl v 'void ()'");
}

/// [dcl.link]: `extern "C"` blocks and single declarations.
#[test]
fn dcl_link_extern_c() {
    let p = parse("extern \"C\" { int a(); }\nextern \"C\" int b();");
    p.assert_clean();
    p.assert_dump("LinkageSpecDecl");
    p.assert_dump("FunctionDecl a 'int ()'");
}

/// [dcl.typedef]: typedef and alias declarations name the same type.
#[test]
fn dcl_typedef_and_alias() {
    let p = parse("typedef unsigned U; using V = U; V v = 1;");
    p.assert_clean();
    p.assert_dump("TypedefDecl referenced U 'unsigned int'");
    p.assert_dump("TypeAliasDecl referenced V 'U':'unsigned int'");
}

/// [dcl.enum]: an unscoped enum without a fixed type picks its best type.
#[test]
fn dcl_enum_unscoped() {
    let p = parse("enum E { A, B = 5, C }; int x = C;");
    p.assert_clean();
    p.assert_dump("EnumDecl E");
    p.assert_dump("EnumConstantDecl referenced C 'E' value=6");
}

/// [dcl.enum]: scoped enumerations do not convert implicitly.
#[test]
fn dcl_enum_scoped_no_implicit_conversion() {
    let p = parse("enum class S : char { X }; int y = S::X;");
    p.assert_diag("cannot initialize a variable of type 'int' with an rvalue of type 'S'");
}

/// [dcl.enum]: an enumerator value must fit a fixed underlying type.
#[test]
fn dcl_enum_fixed_overflow() {
    let p = parse("enum F : unsigned char { Big = 256 };");
    p.assert_diag("error:");
}

/// [dcl.enum]: redefinition of an enumerator.
#[test]
fn dcl_enum_redefinition() {
    let p = parse("enum E { A, A };");
    p.assert_diag("redefinition of enumerator 'A'");
}

/// [class.bit]: bit-field widths and their limits.
#[test]
fn class_bit_width() {
    let p = parse("struct S { unsigned a : 3; int : 0; int b : 40; };");
    p.assert_diag("warning: width of bit-field 'b' (40 bits) exceeds the width of its type; value will be truncated to 32 bits");
}

/// [class.mem]: a struct cannot contain itself.
#[test]
fn class_mem_incomplete_field() {
    let p = parse("struct S { S s; };");
    p.assert_diag("field has incomplete type 'S'");
}

/// [class.union]: unions and anonymous members.
#[test]
fn class_union_anonymous() {
    let p = parse("struct S { union { int i; float f; }; }; int g(S s) { return s.i; }");
    p.assert_clean();
    p.assert_dump("MemberExpr 'int' lvalue .i");
    p.assert_dump("lvalue .\n");
}

/// [basic.def.odr]: variable redefinition.
#[test]
fn basic_def_odr_redefinition() {
    let p = parse("int x = 1; int x = 2;");
    p.assert_diag("redefinition of 'x'");
    p.assert_diag("note: previous definition is here");
}

/// [basic.link]: redeclaration with a different type.
#[test]
fn basic_link_conflicting_types() {
    let p = parse("int f(); long f();");
    p.assert_diag("functions that differ only in their return type cannot be overloaded");
}

/// [dcl.constexpr]: constexpr variables need constant initializers.
#[test]
fn dcl_constexpr_requires_constant() {
    let p = parse("int g(); constexpr int c = g();");
    p.assert_diag("constexpr variable 'c' must be initialized by a constant expression");
}

/// [dcl.constexpr]: constexpr implies const.
#[test]
fn dcl_constexpr_implies_const() {
    let p = parse("constexpr int c = 1;");
    p.assert_clean();
    p.assert_dump("VarDecl c 'const int' constexpr cinit");
}

/// [dcl.pre]: `static_assert` folds its condition.
#[test]
fn dcl_pre_static_assert() {
    let p = parse("static_assert(sizeof(int) == 4); static_assert(1 + 1 == 3, \"math\");");
    p.assert_diag("static assertion failed due to requirement '1 + 1 == 3': math");
}

/// [namespace.def]: nested namespaces and qualified lookup.
#[test]
fn namespace_def_qualified() {
    let p = parse("namespace a::b { int x; } namespace a { int y = b::x; } int z = a::b::x;");
    p.assert_clean();
    p.assert_dump("NamespaceDecl a");
    p.assert_dump("NamespaceDecl b");
}

/// [namespace.udir]: using-directives make names visible.
#[test]
fn namespace_udir() {
    let p = parse("namespace n { int v; } using namespace n; int w = v;");
    p.assert_clean();
    p.assert_dump("UsingDirectiveDecl");
}

/// [basic.lookup]: ambiguous unqualified lookup through two directives.
#[test]
fn basic_lookup_ambiguous() {
    let p = parse(
        "namespace a { int v; } namespace b { int v; }\nusing namespace a; using namespace b;\nint w = v;",
    );
    p.assert_diag("reference to 'v' is ambiguous");
}

/// [namespace.udecl]: a using-declarator list, global and nested
/// qualifiers; references record the shadow they were found through.
#[test]
fn namespace_udecl_declarators() {
    let p = parse(
        "namespace n { int a; int b; namespace m { int c; } }\n\
         using n::a, n::b; using ::n::m::c;\nint w = a + b + c;",
    );
    p.assert_clean();
    p.assert_dump("UsingDecl n::a");
    p.assert_dump("UsingDecl n::b");
    p.assert_dump("UsingDecl ::n::m::c");
    p.assert_dump("DeclRefExpr 'int' lvalue Var 'c' 'int' (UsingShadow 'c')");
}

/// [namespace.udecl]: an unqualified name, a namespace, a missing member.
#[test]
fn namespace_udecl_errors() {
    let p = parse("int x; using x;");
    p.assert_diag("error: using declaration requires a qualified name");
    let p = parse("namespace n { namespace m {} } using n::m;");
    p.assert_diag("error: using declaration cannot refer to a namespace");
    p.assert_diag("note: did you mean 'using namespace'?");
    let p = parse("namespace n {} using n::nope;");
    p.assert_diag("error: no member named 'nope' in namespace 'n'");
    let p = parse("namespace n { int x; } using n::x int y;");
    p.assert_diag("error: expected ';' after using declaration");
    let p = parse("namespace n { int x; } using n::operator+;");
    p.assert_diag("error:");
}

/// [namespace.udecl]/11: conflicts in both directions.
#[test]
fn namespace_udecl_conflicts() {
    let p = parse("namespace n { int v; } int v; using n::v;");
    p.assert_diag("error: target of using declaration conflicts with declaration already in scope");
    let p = parse("namespace n { int v; } using n::v; int v;");
    p.assert_diag("error: declaration conflicts with target of using declaration already in scope");
    let p = parse("namespace n { int v; } void f() { using n::v; int v; }");
    p.assert_diag("note: using declaration");
}

/// clang's `using_if_exists`: silent until the name is used.
#[test]
fn namespace_udecl_using_if_exists() {
    let p = parse("namespace n {} using n::gone __attribute__((__using_if_exists__));");
    p.assert_clean();
    let p = parse("namespace n {} [[clang::using_if_exists]] using n::gone; int k = gone;");
    p.assert_diag("warning: ISO C++ does not allow an attribute list to appear here");
    p.assert_diag("error: reference to unresolved using declaration");
    p.assert_diag("note: using declaration annotated with 'using_if_exists' here");
}

/// [namespace.udecl]/3: in a class, the qualifier must name a base class.
#[test]
fn namespace_udecl_class_scope() {
    let p = parse("namespace n { int x; } struct S { using n::x; };");
    p.assert_diag("error: using declaration in class refers into 'n::', which is not a class");
    let p = parse("struct S { int a; using S::a; };");
    p.assert_diag("error: using declaration refers to its own class");
    let p = parse("struct S { enum E { A }; }; using S::A; int k = A;");
    p.assert_clean();
}

/// [enum.udecl]: the enumerators are introduced as shadows.
#[test]
fn enum_udecl() {
    let p = parse("enum class E { A, B }; void f() { using enum E; E e = B; }");
    p.assert_clean();
    p.assert_dump("UsingEnumDecl Enum 'E'");
    p.assert_dump("EnumConstant 'B' 'E' (UsingShadow 'B')");
    let p = parse("struct S {}; using enum S;");
    p.assert_diag("error: 'S' is not an enumerated type");
    let p = parse("using enum ;");
    p.assert_diag("error: using enum requires an enum or typedef name");
    let p = parse("enum F : int; using enum F;");
    p.assert_diag("error: enumeration 'F' is incomplete");
}

/// [namespace.alias]: aliases, aliases of aliases, redefinitions.
#[test]
fn namespace_alias() {
    let p = parse(
        "namespace a { namespace b { int t; } }\nnamespace ab = a::b; namespace ab2 = ab;\n\
         namespace ab = a::b; int k = ab2::t;\nint f() { namespace l = a; return l::b::t; }",
    );
    p.assert_clean();
    p.assert_dump("NamespaceAliasDecl ab Namespace 'b'");
    p.assert_dump("NamespaceAliasDecl ab2 NamespaceAlias 'ab'");
    p.assert_dump("NamespaceAliasDecl l Namespace 'a'");
    let p = parse("namespace a {} namespace b {} namespace c = a; namespace c = b;");
    p.assert_diag("error: redefinition of 'c' as an alias for a different namespace");
    p.assert_diag("note: previously defined as an alias for 'a'");
    let p = parse("int y; namespace c = y;");
    p.assert_diag("error: expected namespace name");
}

/// [dcl.attr.nodiscard]: discarding a nodiscard result warns.
#[test]
fn dcl_attr_nodiscard() {
    let p = parse("[[nodiscard]] int f(); void g() { f(); }");
    p.assert_diag("warning: ignoring return value of function declared with 'nodiscard' attribute");
}

/// [dcl.attr.deprecated]: using a deprecated entity warns.
#[test]
fn dcl_attr_deprecated() {
    let p = parse("[[deprecated(\"old\")]] int f(); int x = f();");
    p.assert_diag("warning: 'f' is deprecated: old");
}

/// [dcl.align]: alignas raises alignment.
#[test]
fn dcl_align() {
    let p = parse(
        "struct alignas(16) S { char c; }; static_assert(alignof(S) == 16 && sizeof(S) == 16);",
    );
    p.assert_clean();
}

/// GNU `__attribute__`: known attributes apply, unknown ones are ignored
/// with a warning.
#[test]
fn gnu_attribute_unknown_ignored() {
    let p = parse("int x __attribute__((aligned(8))); int y __attribute__((frobnicate));");
    p.assert_dump("AlignedAttr");
    p.assert_diag("warning: unknown attribute 'frobnicate' ignored");
    assert!(p.problems().iter().all(|d| !d.contains("error:")), "{:?}", p.diags);
}

/// [class.union.anon]: anonymous union members are found in the
/// enclosing scope, and anonymous unions outside classes need `static`.
#[test]
fn class_union_anon() {
    let p =
        parse("static union { int a; long b; }; int f() { union { int c; }; c = a; return c; }");
    p.assert_clean();
    let p = parse("union { int a; };");
    p.assert_diag("anonymous unions at namespace or global scope must be declared 'static'");
    let p = parse("int a; static union { int a; };");
    p.assert_diag("member of anonymous union redeclares 'a'");
}

/// [namespace.unnamed]: an unnamed namespace's members are visible in
/// the enclosing scope, and every unnamed namespace there is the same.
#[test]
fn namespace_unnamed() {
    let p = parse("namespace { int q; } namespace { int r = q; } int s = q + r;");
    p.assert_clean();
}

/// [dcl.attr.nodiscard]: a nodiscard class returned by value.
#[test]
fn dcl_attr_nodiscard_type() {
    let p = parse("struct [[nodiscard]] R { int v; }; R make(); void g() { make(); }");
    p.assert_diag("warning: ignoring return value of function declared with 'nodiscard' attribute");
}

/// [dcl.init.list]: narrowing in list-initialization.
#[test]
fn dcl_init_list_narrowing() {
    let p = parse("int x = 1; char c{x};");
    p.assert_diag(
        "non-constant-expression cannot be narrowed from type 'int' to 'char' in initializer list",
    );
}

/// [dcl.init.aggr]: aggregate initialization fills members.
#[test]
fn dcl_init_aggr() {
    let p = parse("struct P { int x, y; }; P p = {1, 2}; int a[3] = {1};");
    p.assert_clean();
    p.assert_dump("InitListExpr 'P'");
}

/// [dcl.init.aggr]: too many initializers.
#[test]
fn dcl_init_aggr_excess() {
    let p = parse("int a[2] = {1, 2, 3};");
    p.assert_diag("excess elements in array initializer");
}

/// [dcl.init.string]: a string too long for its array.
#[test]
fn dcl_init_string_too_long() {
    let p = parse("char s[2] = \"abc\";");
    p.assert_diag(
        "initializer-string for char array is too long, array size is 2 but initializer has size 4",
    );
}

/// Recovery: a missing semicolon after a top-level declaration.
#[test]
fn recovery_missing_semi() {
    let p = parse("int x\nint y;");
    p.assert_diag("expected ';' after top level declarator");
}

/// [dcl.fct.def.general]/1: a function body after a declarator that does
/// not declare a function is diagnosed and skipped, without a crash.
#[test]
fn dcl_fct_def_body_on_non_function() {
    let p = parse("inline constexpr Foo f(Foo v) { return v; }\nint after;");
    p.assert_diag("unknown type name 'Foo'");
    p.assert_dump("VarDecl after 'int'");
}

/// [class.ctor.general]/1: a constructor declaration (the class's own name
/// followed by `(`) is recognised as one and handed to sema, not parsed as a
/// member of the class's own type; [dcl.attr.nodiscard]/1 allows the
/// attribute on a constructor.
#[test]
fn class_ctor_declaration_recognised() {
    let p = parse("struct S { int a; explicit constexpr S(int v); [[nodiscard]] S(); };");
    assert!(p.diags.is_empty(), "{:?}", p.diags);
    p.assert_dump("FieldDecl a 'int'");
}

/// [class.mem.general]/10: a non-static data member shall not have an
/// incomplete type (here, its own class); the member is invalid, and the
/// aggregate initialization of the class does not recurse into it.
#[test]
fn class_mem_self_typed_field() {
    let p = parse("struct N { N n; };\nN x{};\nint after;");
    p.assert_diag("field has incomplete type 'N'");
    p.assert_dump("VarDecl after 'int'");
}

/// [dcl.typedef]/4: a diagnosed typedef redefinition is not the previous
/// declaration of a later one.
#[test]
fn dcl_typedef_invalid_redefinition_not_previous() {
    let p = parse("typedef int T;\ntypedef long T;\ntypedef short T;");
    p.assert_diag("2:14 error: typedef redefinition with different types ('long' vs 'int')");
    p.assert_diag("3:15 error: typedef redefinition with different types ('short' vs 'int')");
}

/// [except.spec]/1-2: exception specifications are part of the function type.
#[test]
fn except_spec_forms() {
    let p = parse(
        "void a() noexcept; void b() noexcept(false); void c() throw();\n\
         void d() noexcept(sizeof(int) == 4); void (*fp)() noexcept = a;",
    );
    p.assert_clean();
    p.assert_dump("FunctionDecl used a 'void () noexcept'");
    p.assert_dump("FunctionDecl b 'void () noexcept(false)'");
    p.assert_dump("FunctionDecl c 'void () throw()'");
    p.assert_dump("FunctionDecl d 'void () noexcept(sizeof(int) == 4)'");
    let p = parse("void a() throw(int);");
    p.assert_diag("ISO C++17 does not allow dynamic exception specifications");
    let p = parse("void a() noexcept(2);");
    p.assert_diag("noexcept specifier argument evaluates to 2");
    let p = parse("void a() noexcept; void a();");
    p.assert_diag("'a' is missing exception specification 'noexcept'");
}

/// [dcl.asm] GNU asm labels: concatenated, stored as `AsmLabelAttr`
/// (first, as clang orders it) and inherited by later redeclarations.
#[test]
fn dcl_asm_label() {
    let p = parse(
        "__attribute__((noinline)) int g() asm(\"g\" \"l\");\n\
         int g();\n\
         extern int v __asm__(\"vl\");\n\
         typedef int T asm(\"ignored\");",
    );
    p.assert_clean();
    p.assert_dump(
        "FunctionDecl g 'int ()'\n| |-AsmLabelAttr \"gl\" IsLiteralLabel\n| `-NoInlineAttr",
    );
    p.assert_dump("FunctionDecl g 'int ()'\n| `-AsmLabelAttr Inherited \"gl\" IsLiteralLabel");
    p.assert_dump("VarDecl v 'int' extern\n| `-AsmLabelAttr \"vl\" IsLiteralLabel");
    p.assert_dump("TypedefDecl T 'int'\n");
}

/// [dcl.asm] GNU asm labels on redeclarations and automatic variables
/// (clang's `mergeDeclAttributes`, `warn_asm_label_on_auto_decl`).
#[test]
fn dcl_asm_label_diagnostics() {
    let p = parse("int f() asm(\"a\");\nint f() asm(\"b\");");
    p.assert_diag("2:5 error: conflicting asm label");
    p.assert_diag("1:13 note: previous declaration is here");
    let p = parse("int f();\nint u() { return f(); }\nint f() asm(\"late\");");
    p.assert_diag("3:5 error: cannot apply asm label to function after its first use");
    let p = parse("int f();\nint f() asm(\"ok\");\nint u() { return f(); }");
    p.assert_clean();
    let p = parse("void f() { int x asm(\"r\"); static int s asm(\"s\"); (void)x; }");
    p.assert_diag("1:22 warning: ignored asm label 'r' on automatic variable");
    let p = parse("int a asm(L\"w\");\nint b asm(u8\"u\");\nint c asm(\"\");\nint d asm(1);");
    p.assert_diag("1:15 error: cannot use wide string literal in 'asm'");
    p.assert_diag("2:16 error: cannot use unicode string literal in 'asm'");
    p.assert_diag("3:13 error: cannot use an empty string literal in 'asm'");
    p.assert_diag("4:11 error: expected string literal in 'asm'");
    let p = parse("int f() asm(\"x\") = 0;");
    p.assert_diag("1:5 error: illegal initializer (only variables can be initialized)");
}
