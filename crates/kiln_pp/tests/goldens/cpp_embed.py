from gen import C
NAME = 'cpp_embed'
TITLE = '[cpp.embed] Resource inclusion through the `IncludeResolver` (here `MemoryResolver`).'
R = 'r'
ABC = {'d.bin': (R, b'abc')}
E = {'e.bin': (R, b'')}
CASES = [
C('embed_basic', '[cpp.embed.gen]/1', 'int a[] = {\n#embed "d.bin"\n};\n', files=ABC),
C('embed_angled', '[cpp.embed.gen]/1', 'int a[] = {\n#embed <d.bin>\n};\n', files=ABC),
C('embed_binary_bytes', '[cpp.embed.gen]/1', 'int a[] = {\n#embed "d.bin"\n};\n', files={'d.bin': (R, b'\x00\xff\n\x7f')}),
C('embed_limit', '[cpp.embed.param.limit]/1', 'int a[] = {\n#embed "d.bin" limit(2)\n};\n', files=ABC),
C('embed_limit_zero_if_empty', '[cpp.embed.param.if.empty]/1',
  'int a[] = {\n#embed "d.bin" limit(0) if_empty(7)\n};\n', files=ABC),
C('embed_limit_macro', '[cpp.embed.param.limit]/1',
  '#define N 1\nint a[] = {\n#embed "d.bin" limit(N)\n};\n', files=ABC),
C('embed_prefix_suffix', ['[cpp.embed.param.prefix]/1', '[cpp.embed.param.suffix]/1'],
  'int a[] = {\n#embed "d.bin" prefix(0, ) suffix(, 9)\n};\n', files=ABC),
C('embed_empty_prefix_suffix_dropped', ['[cpp.embed.param.prefix]/1', '[cpp.embed.param.if.empty]/1'],
  'int a[] = {\n#embed "e.bin" prefix(1, ) suffix(, 2) if_empty(-1)\n};\n', files=E),
C('embed_empty_if_empty', '[cpp.embed.param.if.empty]/1', 'int a[] = {\n#embed "e.bin" if_empty(42)\n};\n', files=E),
C('embed_empty_nothing', '[cpp.embed.gen]/1', 'x\n#embed "e.bin"\ny\n', files=E),
C('embed_clang_prefixed_params', '[cpp.embed.param]/1',
  'int a[] = {\n#embed "d.bin" __limit__(1) clang::offset(1)\n};\n', files=ABC),
C('embed_not_found', '[cpp.embed.gen]/1', '#embed "nope.bin"\nafter\n'),
C('embed_unknown_param', '[cpp.embed.param]/1', '#embed "d.bin" bogus(1)\n', files=ABC),
C('embed_duplicate_param', '[cpp.embed.param]/1', '#embed "d.bin" limit(1) limit(2)\n', files=ABC),
C('embed_negative_limit', '[cpp.embed.param.limit]/1', '#embed "d.bin" limit(-1)\n', files=ABC),
C('embed_macro_name', '[cpp.embed.gen]/1', '#define F "d.bin"\nint a[] = {\n#embed F\n};\n', files=ABC),
C('has_embed_values', '[cpp.cond]/5',
  '#if __has_embed("d.bin") == 1\nfound\n#endif\n#if __has_embed("e.bin") == 2\nempty\n#endif\n'
  '#if __has_embed("nope.bin") == 0\nmissing\n#endif\n#if __has_embed("d.bin" limit(0)) == 2\nlimit0\n#endif\n'
  '#if __has_embed("d.bin" bogus) == 0\nunsupported\n#endif\n',
  files={'d.bin': (R, b'abc'), 'e.bin': (R, b'')}),
C('has_embed_defined', '[cpp.cond]/5', '#ifdef __has_embed\nyes\n#endif\n'),
]
