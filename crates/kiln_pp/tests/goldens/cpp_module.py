from gen import C
NAME = 'cpp_module'
TITLE = '[cpp.module] / [cpp.import] Module and import directives (printed as text; markers are checked in `hooks.rs`).'
CASES = [
C('module_decl_text', '[cpp.module]/1', 'export module m.n:p;\nimport q;\nint x;\n'),
C('global_module_fragment', '[cpp.module]/1', 'module;\n#define A 1\nexport module m;\nint y = A;\n'),
C('module_tokens_expanded', '[cpp.module]/3', '#define ATTR [[]]\nexport module m ATTR;\n'),
C('module_name_is_macro', '[cpp.module]/2', '#define m x\nexport module m;\n',
  diags=['2:15: error: module name component \'m\' cannot be an object-like macro'],
  note='Deviation: [cpp.module]/2 makes this ill-formed; clang 19 does not diagnose it.'),
C('import_header_unit', '[cpp.import]/1', 'import <vector>;\nimport "a.h";\n',
  diags=['1:8: error: importing a header unit is not supported yet', '2:8: error: importing a header unit is not supported yet'],
  note='Deviation: header units are not supported (clang looks the header up, then rejects it).'),
C('module_not_at_line_start', '[cpp.module]/1', 'int module; x module m;\n'),
C('module_as_identifier', '[cpp.module]/1', 'module = 1;\nimport + 2;\nexport { }\n'),
C('import_private_partition', '[cpp.import]/1', 'module :private;\nimport :p;\n'),
]
