from gen import C
NAME = 'cpp_options'
TITLE = 'Driver options of the preprocessor (`PpOptions`): `-C`, `-CC`, `-D`, `-U`, include depth; [cpp.predefined]/4.'
H = 'h'
CASES = [
C('keep_comments', '[lex.comment]/1', 'a /* c1 */ b // c2\n/* multi\n   line */ c\n',
  args=['-C'], cfg='|o| o.keep_comments = true'),
C('keep_comments_not_in_macros', '[lex.comment]/1', '#define M a /* in */ b\nM // tail\n',
  args=['-C'], cfg='|o| o.keep_comments = true'),
C('keep_macro_comments', '[lex.comment]/1', '#define M a /* in */ b\nM // tail\n',
  args=['-CC'], cfg='|o| o.keep_macro_comments = true', want='\na b // tail\n',
  note='Deviation: comments in `#define` bodies are not retained (clang prints `a /* in */ b`).'),
C('keep_macro_comments_in_args', '[lex.comment]/1', '#define F(x) [x]\nF(a /* c */ b)\n',
  args=['-CC'], cfg='|o| o.keep_macro_comments = true'),
C('command_line_define', '[cpp.predefined]/4', 'A B C\n#ifdef U\nu\n#endif\n',
  args=['-DA', '-DB=two', '-DC(x)=x', '-DU', '-UU'],
  cfg='|o| {\n            o.defines = vec![\n                ("A".into(), None),\n                ("B".into(), Some("two".into())),\n'
      '                ("C(x)".into(), Some("x".into())),\n                ("U".into(), None),\n            ];\n'
      '            o.undefines = vec!["U".into()];\n        }'),
C('include_depth_limit', '[cpp.include]/6', '#include "r.h"\n',
  files={'r.h': (H, '#include "r.h"\n')}, cfg='|o| o.max_include_depth = 3',
  diags=['1:10: error: #include nested too deeply'],
  note='clang 19 uses its default limit of 200 here; the text and diagnostic do not depend on it.'),
]
