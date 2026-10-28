from gen import C
NAME = 'cpp_replace'
TITLE = '[cpp.replace] Macro replacement: definition, redefinition, invocation.'
CASES = [
C('object_like', '[cpp.replace.general]/9', '#define A 1 + 2\nint x = A;\n'),
C('object_like_empty', '[cpp.replace.general]/9', '#define E\n[E]\n'),
C('function_like', '[cpp.replace.general]/10', '#define F(a, b) a * b\nF(1, 2) F (3,4)\n'),
C('function_like_without_parens', '[cpp.replace.general]/10', '#define F(a) a\nF + F\nint F;\n'),
C('invocation_spans_lines', '[cpp.replace.general]/11', '#define F(a, b) [a|b]\nF(\n1\n,\n2\n) after\n'),
C('nested_parens_in_args', '[cpp.replace.general]/11', '#define F(a, b) [a|b]\nF((1, 2), (3))\n'),
C('empty_arguments', '[cpp.replace.general]/11', '#define F(a, b) [a|b]\nF(,) F(1,) F(,2)\n#define G(a) [a]\nG() G( )\n'),
C('too_many_arguments', '[cpp.replace.general]/12', '#define F(a) a\nF(1, 2)\n'),
C('too_few_arguments', '[cpp.replace.general]/12', '#define F(a, b) a b\nF(1)\n'),
C('unterminated_invocation', '[cpp.replace.general]/11', '#define F(a) a\nF(1\n'),
C('directive_inside_arguments', '[cpp.replace.general]/12',
  '#define F(a) [a]\nF(1\n#define X 2\nX)\nX\n'),
C('variadic', '[cpp.replace.general]/13', '#define V(a, ...) a: __VA_ARGS__\nV(1) V(1, 2, 3) V(1,)\n'),
C('variadic_only', '[cpp.replace.general]/13', '#define V(...) <__VA_ARGS__>\nV() V(x) V(x, (y, z))\n'),
C('gnu_named_variadic', '[cpp.replace.general]/13', '#define V(args...) <args>\nV(1, 2)\n'),
C('va_args_outside_variadic', '[cpp.replace.general]/14', '#define A __VA_ARGS__\n'),
C('identical_redefinition', '[cpp.replace.general]/2', '#define A 1  +  2\n#define A 1 + 2\n#define F(x) x\n#define F( x ) x\nA F(3)\n'),
C('incompatible_redefinition', '[cpp.replace.general]/2', '#define A 1 + 2\n#define A 1+2\n#define B 1\n#define B 2\n#define F(x) x\n#define F(y) y\nA B F(3)\n'),
C('duplicate_parameter', '[cpp.replace.general]/6', '#define F(a, a) a\n'),
C('missing_whitespace_after_name', '[cpp.replace.general]/4', '#define A+1\nA\n'),
C('missing_macro_name', '[cpp.replace.general]/1', '#define\n#define 3\n#undef\n'),
C('define_defined', '[cpp.replace.general]/8', '#define defined\n#undef defined\n'),
C('keyword_macro', '[cpp.replace.general]/8', '#define int long\nint x;\n'),
C('undef', '[cpp.scope]/2', '#define A 1\nA\n#undef A\nA\n#undef A\n#undef B extra\n'),
C('builtin_redefinition', '[cpp.predefined]/4', '#define __LINE__ 1\n#undef __FILE__\n'),
C('predefined_undef', '[cpp.predefined]/4', '#undef __cplusplus\n__cplusplus\n'),
C('function_like_name_as_object', '[cpp.replace.general]/10', '#define f(a) a*g\n#define g(a) f(a)\nf(2)(9)\n'),
C('bad_parameter_list', '[cpp.replace.general]/1', '#define F(a b) a\n#define G(a,) a\n#define H(1) a\n#define I(a\n'),
C('va_opt_basic', '[cpp.subst]/3', '#define F(a, ...) f(a __VA_OPT__(,) __VA_ARGS__)\nF(1) F(1, 2) F(1, )\n'),
C('va_opt_empty_expansion', '[cpp.subst]/3',
  '#define E\n#define F(...) [__VA_OPT__(x)]\nF() F(E) F(E E) F(1)\n'),
C('va_opt_stringize_and_paste', '[cpp.subst]/3',
  '#define S(...) #__VA_OPT__(a b)\n#define P(x, ...) x ## __VA_OPT__(y) ## z\nS() S(1) P(1) P(1, 2)\n'),
C('va_opt_outside_variadic', '[cpp.subst]/3', '#define F(a) __VA_OPT__(a)\n'),
C('va_opt_unterminated', '[cpp.subst]/3', '#define F(...) __VA_OPT__(a\n'),
C('va_opt_nested', '[cpp.subst]/3', '#define F(...) __VA_OPT__(__VA_OPT__())\n'),
C('va_opt_standard_example', '[cpp.subst]/5',
  '#define F(...) f(0 __VA_OPT__(,) __VA_ARGS__)\n#define G(X, ...) f(0, X __VA_OPT__(,) __VA_ARGS__)\n'
  '#define SDEF(sname, ...) S sname __VA_OPT__(= { __VA_ARGS__ })\n#define EMP\n'
  'F(a,b,c)\nF()\nF(EMP)\nG(a,b,c)\nG(a,)\nG(a)\nSDEF(foo);\nSDEF(bar, 1, 2);\n'
  '#define H2(X, Y, ...) __VA_OPT__(X ## Y,) __VA_ARGS__\nH2(a, b, c, d)\n'
  '#define H3(X, ...) #__VA_OPT__(X##X X##X)\nH3(, 0)\n'
  '#define H4(X, ...) __VA_OPT__(a X ## X) ## b\nH4(, 1)\n'
  '#define H5A(...) __VA_OPT__()/**/__VA_OPT__()\n#define H5B(X) a ## X ## b\n#define H5C(X) H5B(X)\nH5C(H5A())\n'),
C('arguments_fully_expanded', '[cpp.subst]/1',
  '#define A 1\n#define F(x) [x]\n#define G(x) F(x)\nG(A) F(A)\n'),
C('argument_with_unbalanced_macro', '[cpp.subst]/1',
  '#define LP (\n#define F(x) x\n#define G(x) F x 1)\nG(LP)\n'),
]
