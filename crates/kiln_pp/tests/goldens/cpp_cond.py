from gen import C
NAME = 'cpp_cond'
TITLE = '[cpp.cond] Conditional inclusion: group structure and skipping (expressions via the stub evaluator).'
CASES = [
C('if_true_false', '[cpp.cond]/1', '#if 1\na\n#endif\n#if 0\nb\n#endif\n'),
C('else_branches', '[cpp.cond]/1', '#if 0\na\n#else\nb\n#endif\n#if 1\nc\n#else\nd\n#endif\n'),
C('elif_chain', '[cpp.cond]/1', '#define X 2\n#if X == 1\na\n#elif X == 2\nb\n#elif X == 2\nc\n#else\nd\n#endif\n'),
C('ifdef_ifndef', '[cpp.cond]/14', '#define A\n#ifdef A\na\n#endif\n#ifndef A\nb\n#endif\n#ifdef B\nc\n#endif\n#ifndef B\nd\n#endif\n'),
C('elifdef_elifndef', '[cpp.cond]/14', '#define B\n#if 0\na\n#elifdef B\nb\n#endif\n#ifdef A\nc\n#elifndef A\nd\n#endif\n'),
C('defined_operator', '[cpp.cond]/10', '#define A\n#if defined A && defined(A) && !defined B\nyes\n#endif\n'),
C('nested_skipped_groups', '[cpp.cond]/1',
  '#if 0\n#if garbage (\n#else\n#elif\n#endif\nx\n#else\ny\n#endif\n'),
C('skipped_group_ignores_bad_directives', '[cpp.cond]/1',
  '#if 0\n#bogus\n#define\n#include\n#error no\n\'unterminated\n#endif\nok\n'),
C('skipped_elif_not_evaluated', '[cpp.cond]/1', '#if 1\na\n#elif 1/0\nb\n#endif\n'),
C('undefined_identifier_is_zero', '[cpp.cond]/13', '#if UNDEFINED\na\n#else\nb\n#endif\n'),
C('true_false_keywords', '[cpp.cond]/13', '#if true && !false\na\n#endif\n'),
C('arithmetic', '[cpp.cond]/13',
  '#if (1 + 2 * 3 == 7) && (-1 < 0) && (0x10 == 16) && (010 == 8) && (1\'000 == 1000) && (5 % 3 == 2) && (1 << 4 == 16)\nok\n#endif\n'
  '#if (1 ? 2 : 3) == 2 && (0 || 1) && ~0 == -1\nok2\n#endif\n'),
C('else_after_else', '[cpp.cond]/1', '#if 1\n#else\n#else\n#endif\n'),
C('elif_after_else', '[cpp.cond]/1', '#if 0\n#else\n#elif 1\n#endif\n'),
C('endif_without_if', '[cpp.cond]/1', '#endif\n#else\n#elif 1\n'),
C('unterminated_conditional', '[cpp.cond]/1', '#if 1\na\n'),
C('missing_expression', '[cpp.cond]/1', '#if\n#endif\n#ifdef\n#endif\n'),
C('extra_tokens_after_endif', '[cpp.cond]/1', '#if 1\n#else junk\n#endif junk\n#ifdef A junk\n#endif\n'),
C('ifdef_non_identifier', '[cpp.cond]/14', '#ifdef 3\n#endif\n'),
C('defined_in_macro', '[cpp.cond]/13', '#define D defined(X)\n#if D\na\n#else\nb\n#endif\n'),
C('defined_in_function_like_macro', '[cpp.cond]/13', '#define D(x) defined(x)\n#if D(X)\na\n#else\nb\n#endif\n'),
C('has_include_in_skipped', '[cpp.cond]/1', '#if 0\n#if __has_include(<nope>)\n#endif\n#endif\nok\n'),
C('conditional_in_macro_args', '[cpp.cond]/1', '#define F(x) [x]\nF(a\n#if 1\nb\n#else\nc\n#endif\n)\n'),
C('division_by_zero', '[cpp.cond]/13', '#if 1 / 0\n#endif\n'),
C('missing_paren', '[cpp.cond]/13', '#if (1\n#endif\n'),
]
