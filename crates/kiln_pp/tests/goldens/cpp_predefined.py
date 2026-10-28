from gen import C
NAME = 'cpp_predefined'
TITLE = '[cpp.predefined] Predefined macro names and the builtin function-like macros the core answers.'
CASES = [
C('cplusplus', '[cpp.predefined]/1', '__cplusplus __STDC_HOSTED__ __STDCPP_DEFAULT_NEW_ALIGNMENT__ __STDCPP_THREADS__\n'),
C('feature_test_macros', '[cpp.predefined]/1', '__cpp_constexpr __cpp_concepts __cpp_pack_indexing __cpp_static_assert __cpp_variadic_friend\n'),
C('file_and_line', '[cpp.predefined]/1', '__FILE__ __LINE__\n\n__LINE__ __FILE_NAME__ __BASE_FILE__\n'),
C('line_continued_line', '[cpp.predefined]/1', 'a \\\n__LINE__\n'),
C('line_in_macro_argument', '[cpp.predefined]/1', '#define F(x, y) x y __LINE__\nF(__LINE__,\n__LINE__\n)\n'),
C('counter', '[cpp.predefined]/1', '__COUNTER__ __COUNTER__\n#define C __COUNTER__\nC C\n'),
C('include_level', '[cpp.predefined]/1', '__INCLUDE_LEVEL__\n#include "a.h"\n', files={'a.h': ('h', '__INCLUDE_LEVEL__ __FILE_NAME__ __BASE_FILE__\n')}),
C('flt_eval_method', '[cpp.predefined]/1', '__FLT_EVAL_METHOD__\n'),
C('has_include', '[cpp.cond]/2',
  '#if __has_include("a.h") && __has_include(<a.h>) && !__has_include("nope.h")\nyes\n#endif\n'
  '#define HDR <a.h>\n#if __has_include(HDR)\nmacro\n#endif\n#ifdef __has_include\ndefined\n#endif\n',
  files={'a.h': ('h', '')}),
C('has_include_outside_if', '[cpp.cond]/2', '__has_include("a.h")\n'),
C('has_include_malformed', '[cpp.cond]/2', '#if __has_include\n#endif\n#if __has_include(\n#endif\n#if __has_include("a.h"\n#endif\n'),
C('has_cpp_attribute', '[cpp.cond]/5',
  '__has_cpp_attribute(nodiscard) __has_cpp_attribute(maybe_unused) __has_cpp_attribute(assume) __has_cpp_attribute(likely) __has_cpp_attribute(__noreturn__) __has_cpp_attribute(bogus)\n'),
C('has_cpp_attribute_malformed', '[cpp.cond]/5', '__has_cpp_attribute(1)\n__has_cpp_attribute nodiscard\n'),
C('is_identifier', '[cpp.predefined]/1', '__is_identifier(int) __is_identifier(foo) __is_identifier(and) __is_identifier(co_await) __is_identifier(contract_assert)\n',
  want='                   0 1 0 0 0\n',
  note='Deviation: `contract_assert` is a C++26 keyword (P2900); clang 19 predates it and gives 1.'),
C('date_time_shape', '[cpp.predefined]/1', '#define S(x) #x\n#define X(x) S(x)\nX(__DATE__) X(__TIME__)\n',
  want='"\\"Jan  1 2000\\"" "\\"00:00:00\\""\n',
  note='`PpOptions::reproducible_time` replaces the clock (clang `-ffixed-date`-like).'),
C('builtin_macro_defined', '[cpp.predefined]/4', '#if defined(__LINE__) && defined(__has_include) && defined(__COUNTER__) && defined(_Pragma)\nyes\n#endif\n'),
C('undef_builtin', '[cpp.predefined]/4', '#undef __LINE__\n__LINE__\n'),
C('has_include_next_in_main', '[cpp.cond]/2', '#if __has_include_next("a.h")\nyes\n#endif\n', files={'a.h': ('h', '')}),
C('building_module', '[cpp.predefined]/4',
  '#ifdef __building_module\ndefined\n#endif\n#if __building_module(foo)\nyes\n#else\nno\n#endif\na __building_module(foo) b\nc __building_module(1) d\n',
  note='clang always registers `__building_module`; kiln builds no Clang modules, so it is 0.'),
]
