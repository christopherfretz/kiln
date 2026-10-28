//@ expect: pp
// Raw string literals spanning lines inside macro arguments: the raw
// string is one token (commas and parentheses inside it do not split
// arguments), and phase 1-2 transformations are reverted inside it.
#define ID(x) x
#define FIRST(a, ...) a
#define STR(x) #x
ID(R"(line one
line two, with (parens) and a comma)") end1;
FIRST(R"delim(a, b
  )" not the end )delim", second) end2;
STR(R"x(spliced\
line)x") end3;
ID(u8R"(utf8
raw)" LR"(wide)" uR"()" UR"(
)") end4;
STR(R"(
)") end5;
