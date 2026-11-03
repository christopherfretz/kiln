//@ expect: ast
//@ ast-filter: C
// [temp.arg.nontype], [lex.ccon]: a character-typed template argument in a
// specialization's qualified name prints as clang's CharacterLiteral::print
// spells it: the encoding prefix, the simple escapes (\\ \' \a \b \f \n \r
// \t \v), printable ASCII verbatim, else \xhh below 256, \uhhhh up to 0xffff
// and \Uhhhhhhhh above (lowercase, zero-padded). A deduced (auto) parameter
// prints the cast of signed char / unsigned char; a declared one does not.
template <auto v> struct Y { struct C { int y; }; };
template <> struct Y<(char)0xc8>::C { int a; };
template <> struct Y<'\n'>::C { int b; };
template <> struct Y<'\0'>::C { int c; };
template <> struct Y<'\''>::C { int d; };
template <> struct Y<'\\'>::C { int e; };
template <> struct Y<u8'a'>::C { int f; };
template <> struct Y<(char8_t)0xe9>::C { int g; };
template <> struct Y<u'é'>::C { int h; };
template <> struct Y<U'\U0001F600'>::C { int i; };
template <> struct Y<L'\x1234'>::C { int j; };
template <> struct Y<L'z'>::C { int k; };
template <> struct Y<(signed char)-3>::C { int l; };
template <> struct Y<(unsigned char)200>::C { int m; };
template <> struct Y<'\x7f'>::C { int n; };
template <> struct Y<'\t'>::C { int o; };
template <> struct Y<'"'>::C { int p; };
template <> struct Y<u'\xd800'>::C { int q; };
template <> struct Y<'\a'>::C { int r; };
template <> struct Y<U'\x7f'>::C { int s; };
template <> struct Y<u8'\x80'>::C { int t; };
template <char v> struct A { struct C { int y; }; };
template <> struct A<'\310'>::C { int a; };
template <> struct A<'\101'>::C { int b; };
template <> struct A<'\0'>::C { int c; };
template <> struct A<'\\'>::C { int d; };
template <signed char v> struct B { struct C { int y; }; };
template <> struct B<-3>::C { int a; };
template <> struct B<65>::C { int b; };
template <unsigned char v> struct D { struct C { int y; }; };
template <> struct D<200>::C { int a; };
template <char8_t v> struct E { struct C { int y; }; };
template <> struct E<u8'\xe9'>::C { int a; };
template <char16_t v> struct F { struct C { int y; }; };
template <> struct F<u'\x1234'>::C { int a; };
template <char32_t v> struct G { struct C { int y; }; };
template <> struct G<U'\U0001F600'>::C { int a; };
template <> struct G<U'\n'>::C { int b; };
template <wchar_t v> struct H { struct C { int y; }; };
template <> struct H<L'\xe9'>::C { int a; };
template <> struct H<L'\''>::C { int b; };
template <> struct F<u'\xd800'>::C { int b; };
template <> struct F<u'\xff'>::C { int c; };
template <> struct H<L'\U0001F600'>::C { int c; };
template <> struct H<(wchar_t)-1>::C { int d; };
template <> struct E<u8'\x7f'>::C { int b; };
