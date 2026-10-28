from gen import C
NAME = 'cpp_rescan'
TITLE = '[cpp.rescan] Rescanning and further replacement; [cpp.scope].'
CASES = [
C('rescan_object', '[cpp.rescan]/1', '#define A B\n#define B C\n#define C 1\nA\n'),
C('self_reference_object', '[cpp.rescan]/2', '#define A A + 1\n#define B C\n#define C B\nA B C\n'),
C('self_reference_function', '[cpp.rescan]/2', '#define f(x) f(x) + 1\nf(f(1))\n'),
C('painted_blue_stays_blue', '[cpp.rescan]/2', '#define foo foo\n#define bar(x) x\nbar(foo)\n'),
C('function_name_from_rescan', '[cpp.rescan]/1', '#define F(x) [x]\n#define G F\n#define H G(1)\nG(2) H G\n(3)\n'),
C('function_name_at_end_of_expansion', '[cpp.rescan]/1', '#define F(x) [x]\n#define G F\nG (1) G\nnot_paren\n'),
C('arguments_from_outside_expansion', '[cpp.rescan]/1',
  '#define f(a) a*g\n#define g(a) f(a)\nf(2)(9)\n'),
C('nested_self_reference', '[cpp.rescan]/2',
  '#define A(x) B(x)\n#define B(x) A(x)\nA(1) B(A(1))\n'),
C('directive_like_result_not_directive', '[cpp.rescan]/3', '#define EMPTY\n#define D # define X 1\nD\nEMPTY # define Y\n'),
C('pragma_in_expansion_not_directive', '[cpp.rescan]/3', '#define P #pragma x\nP\n'),
C('obj_with_paren_after', '[cpp.rescan]/1', '#define A (x)\n#define F(y) <y>\nF A\n'),
C('deep_nesting', '[cpp.rescan]/1',
  '#define A0 x\n#define A1 A0 A0\n#define A2 A1 A1\n#define A3 A2 A2\n#define A4 A3 A3\nA4\n'),
C('scope_undef_redefine', '[cpp.scope]/1', '#define A 1\nA\n#undef A\n#define A 2\nA\n'),
C('macro_in_directive_line', '[cpp.scope]/1', '#define H "x"\n#line 10 H\n__FILE__ __LINE__\n'),
]
