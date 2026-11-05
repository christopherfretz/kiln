// [lex.charset]/2, [lex.name] (C++11, N2170): universal-character-names in
// identifiers and literals. WP-646 feature probe.
//@ expect: run
//@ std: c++11
//@ strict-diags
//@ stdout: 7 233 3
extern "C" int printf(const char*, ...);
int été = 7;
static_assert(sizeof("é") == 3, "UCN in a narrow literal is UTF-8");
static_assert(U'é' == 0xe9 && U"\U000000E9"[0] == 0xe9, "UCN value");
int main() {
  printf("%d %d %d\n", été, (int)U'é', (int)sizeof(u8"é"));
}
