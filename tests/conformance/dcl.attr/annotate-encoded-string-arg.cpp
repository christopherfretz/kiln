// clang's `annotate` attribute (WP-1113, [dcl.attr.grammar]/5,
// [lex.string]): only the annotation (the first argument) is an
// unevaluated string ([lex.string.uneval]); the other arguments are
// expressions, so a string literal there may have an encoding prefix, be
// concatenated, or be subscripted.
//@ expect: compile-ok
//@ strict-diags
int a [[clang::annotate("x", u8"s", L"w", u"a" u"b", U"c", "d"[0])]];
[[clang::annotate("f", L"w")]] void f() {}
