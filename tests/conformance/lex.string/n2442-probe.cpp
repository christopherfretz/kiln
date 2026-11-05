// [lex.string] (C++11, N2442 raw string literals; N2249 u/U literals and
// char16_t/char32_t; N2442 u8 literals). WP-646 feature probe.
//@ expect: run
//@ std: c++11
//@ strict-diags
//@ stdout: 5 3 4 2 2 4 3
extern "C" int printf(const char*, ...);
const char raw[] = R"(a\n"b)";
const char delim[] = R"xy()")xy";
const char nl[] = R"(
x)";
static_assert(sizeof(raw) == 6, "raw keeps backslashes and quotes");
static_assert(sizeof(u"ab") == 3 * sizeof(char16_t), "u literal");
static_assert(sizeof(U"ab") == 3 * sizeof(char32_t), "U literal");
static_assert(sizeof(u8"é") == 3, "u8 encodes UTF-8");
static_assert(sizeof(LR"(x)") == 2 * sizeof(wchar_t), "LR literal");
static_assert(u'é' == 0xe9 && U'\U0001F600' == 0x1F600, "char16/32");
int main() {
  printf("%d %d %d %d %d %d %d\n", (int)sizeof(raw) - 1, (int)sizeof(delim),
         (int)sizeof(u8R"(ab)") + 1, (int)sizeof(nl) - 1,
         (int)(sizeof(u"x") / sizeof(char16_t)), (int)sizeof(char32_t),
         (int)sizeof(u8"é"));
}
