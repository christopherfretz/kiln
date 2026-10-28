from gen import C
NAME = 'cpp_stringize'
TITLE = '[cpp.stringize] The # operator and [cpp.concat] the ## operator.'
CASES = [
C('stringize_basic', '[cpp.stringize]/2', '#define S(x) #x\nS(a) S( a  b ) S() S(a\n b)\n'),
C('stringize_escapes', '[cpp.stringize]/2', '#define S(x) #x\nS("a\\n" \'"\' \'\\\'\' L"x" u8"\\\\")\n'),
C('stringize_raw_string', '[cpp.stringize]/2', '#define S(x) #x\nS(R"(a"\\b)")\n'),
C('stringize_unexpanded', '[cpp.stringize]/2', '#define A 1\n#define S(x) #x\n#define X(x) S(x)\nS(A) X(A)\n'),
C('stringize_variadic', '[cpp.stringize]/2', '#define S(...) #__VA_ARGS__\nS() S(a,b) S(a , b)\n'),
C('stringize_non_parameter', '[cpp.stringize]/1', '#define S(x) #y\n#define T(x) # 1\n'),
C('stringize_object_like', '[cpp.stringize]/1', '#define S # x\nS\n'),
C('stringize_comments', '[cpp.stringize]/2', '#define S(x) #x\nS(a/**/b /* c */ d // e\n)\n'),
C('stringize_digraph', '[cpp.stringize]/2', '#define S(x) %:x\nS(<: :>)\n'),
C('concat_basic', '[cpp.concat]/3', '#define C(a, b) a ## b\nC(x, y) C(1, 2) C(+, =) C(<, <=) C(., 5)\n'),
C('concat_empty', '[cpp.concat]/3', '#define C(a, b) [a ## b]\nC(,) C(x,) C(,y)\n'),
C('concat_placemarker_chain', '[cpp.concat]/3', '#define C3(a, b, c) a ## b ## c\nC3(,,) C3(x,,) C3(,,z) C3(x,,z)\n'),
C('concat_result_rescanned', '[cpp.concat]/3', '#define xy 42\n#define C(a, b) a ## b\nC(x, y)\n'),
C('concat_invalid', '[cpp.concat]/3', '#define C(a, b) a ## b\nC(+, -) C(x, +)\n'),
C('concat_at_edges', '[cpp.concat]/1', '#define A ## x\n#define B x ##\n#define F(a) ## a\n'),
C('concat_not_expanded', '[cpp.concat]/2', '#define A 1\n#define C(a, b) a ## b\n#define X(a, b) C(a, b)\nC(A, A) X(A, A)\n'),
C('concat_stringize_order', '[cpp.concat]/4',
  '#define hash_hash # ## #\n#define mkstr(a) # a\n#define in_between(a) mkstr(a)\n#define join(c, d) in_between(c hash_hash d)\nchar p[] = join(x, y);\n'),
C('concat_gnu_comma', '[cpp.concat]/3',
  '#define E(fmt, ...) f(fmt, ## __VA_ARGS__)\nE(a) E(a, b) E(a,)\n#define G(fmt, args...) f(fmt, ## args)\nG(a) G(a, b)\n'),
C('concat_ud_literals', '[cpp.concat]/3', '#define C(a, b) a ## b\nC(1, _x) C("s", _y) C(L, "w") C(u8, \'c\')\n'),
C('concat_creates_comment', '[cpp.concat]/3', '#define C(a, b) a ## b\nC(/, /) C(/, *)\n'),
C('standard_example_1', '[cpp.scope]/5',
  '#define x 3\n#define f(a) f(x * (a))\n#undef x\n#define x 2\n#define g f\n#define z z[0]\n#define h g(~\n#define m(a) a(w)\n#define w 0,1\n#define t(a) a\n#define p() int\n#define q(x) x\n#define r(x,y) x ## y\n#define str(x) # x\n'
  'f(y+1) + f(f(z)) % t(t(g)(0) + t)(1);\ng(x+(3,4)-w) | h 5) & m\n(f)^m(m);\np() i[q()] = { q(1), r(2,3), r(4,), r(,5), r(,) };\nchar c[2][6] = { str(hello), str() };\n'),
C('standard_example_2', '[cpp.scope]/6',
  '#define str(s) # s\n#define xstr(s) str(s)\n#define debug(s, t) printf("x" # s "= %d, x" # t "= %s", \\\n x ## s, x ## t)\n#define INCFILE(n) vers ## n\n#define glue(a, b) a ## b\n#define xglue(a, b) glue(a, b)\n#define HIGHLOW "hello"\n#define LOW LOW ", world"\n'
  'debug(1, 2);\nfputs(str(strncmp("abc\\0d", "abc", \'\\4\') // this goes away\n == 0) str(: @\\n), s);\nxstr(INCFILE(2).h)\nglue(HIGH, LOW);\nxglue(HIGH, LOW)\n'),
C('standard_example_4', '[cpp.scope]/8',
  '#define t(x,y,z) x ## y ## z\nint j[] = { t(1,2,3), t(,4,5), t(6,,7), t(8,9,),\nt(10,,), t(,11,), t(,,12), t(,,) };\n'),
C('standard_example_5', '[cpp.scope]/9',
  '#define OBJ_LIKE (1-1)\n#define OBJ_LIKE /* white space */ (1-1) /* other */\n#define FUNC_LIKE(a) ( a )\n#define FUNC_LIKE( a )( /* note the white space */ \\\na /* other stuff on this line\n*/ )\n'
  '#define OBJ_LIKE (0)\n#define OBJ_LIKE (1 - 1)\n#define FUNC_LIKE(b) ( a )\n#define FUNC_LIKE(b) ( b )\n'),
C('standard_example_6', '[cpp.scope]/10',
  '#define debug(...) fprintf(stderr, __VA_ARGS__)\n#define showlist(...) puts(#__VA_ARGS__)\n#define report(test, ...) ((test)?puts(#test):\\\n printf(__VA_ARGS__))\n'
  'debug("Flag");\ndebug("X = %d\\n", x);\nshowlist(The first, second, and third items.);\nreport(x>y, "x is %d but y is %d", x, y);\n'),
]
