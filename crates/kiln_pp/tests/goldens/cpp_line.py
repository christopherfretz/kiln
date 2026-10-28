from gen import C
NAME = 'cpp_line'
TITLE = '[cpp.line] Line control; [cpp.error] diagnostic directives; [cpp.null].'
CASES = [
C('line_number', '[cpp.line]/3', 'a __LINE__\n#line 100\nb __LINE__\nc __LINE__\n'),
C('line_number_and_file', '[cpp.line]/4', '#line 7 "other.cpp"\n__LINE__ __FILE__\n'),
C('line_macro_expanded', '[cpp.line]/5', '#define N 42\n#define F "f.cpp"\n#line N F\n__LINE__ __FILE__\n'),
C('line_out_of_range', '[cpp.line]/3', '#line 0\n#line 2147483648\n#line 4294967296\n'),
C('line_not_digit_sequence', '[cpp.line]/3', '#line 0x10\n#line 1u\n#line -1\n#line a\n'),
C('line_bad_filename', '[cpp.line]/4', '#line 5 foo\n#line 6 L"w"\n#line 7 "a" extra\n'),
C('line_missing_number', '[cpp.line]/3', '#line\n'),
C('line_marker_gnu', '[cpp.line]/5', '# 33 "g.cpp"\n__LINE__ __FILE__\n# 40 "h.h" 1 3\n__LINE__ __FILE__\n'),
C('line_marker_bad_flags', '[cpp.line]/5', '# 1 "a" 5\n# 1 "a" 2\n'),
C('line_marker_push_pop', '[cpp.line]/5', '# 1 "a.h" 1\n__FILE__ __LINE__\n# 5 "t.cpp" 2\n__FILE__ __LINE__\n# 9 "t.cpp" 2\n'),
C('line_in_multiline_invocation', '[cpp.predefined]/1',
  '#define F(x) __LINE__ x\nF(\n1\n)\n__LINE__\n'),
C('line_in_nested_multiline_invocation', '[cpp.predefined]/1',
  '#define F(x) x\n#define G(x) F(x __LINE__)\nG(a\n)\nF(__LINE__\n)\n'),
C('error_directive', '[cpp.error]/1', '#error this is "bad" \'x\'\nafter\n'),
C('warning_directive', '[cpp.error]/1', '#warning careful  now\nafter\n'),
C('error_empty', '[cpp.error]/1', '#error\n#warning\n'),
C('error_unterminated_literal', '[cpp.error]/1', '#error don\'t\n'),
C('null_directive', '[cpp.null]/1', '#\n# /* comment */\nx\n'),
C('unknown_directive', '[cpp.pre]/1', '#frobnicate\n#123\n#"x"\n'),
C('ident_directive', '[cpp.pre]/1', '#ident "v1"\n#sccs "v2"\n#ident v3\n'),
C('assert_unassert', '[cpp.pre]/1', '#assert machine(x)\n#unassert machine\n'),
C('conditionally_supported_directive', '[cpp.pre]/1', '#if 0\n#frobnicate\n#endif\nx\n'),
]
