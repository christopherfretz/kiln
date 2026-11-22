// The written-reference table (spec/ast.md §18.5): a name from a macro
// body or a macro argument keeps the location the lexer gave its token,
// so the dump shows its spelling position and (`exp=`) the point of the
// outermost expansion. A name pasted together by `##` is spelled in the
// scratch buffer.
//@ written-refs: macros.refs
namespace cfg {
int level = 2;
int bump(int v) { return v + 1; }
} // namespace cfg
#define LEVEL cfg::level
#define BUMP(x) cfg::bump(x)
#define TWICE(x) (x + x)
#define GLUE(a, b) a##b

int local_value = 5;

int macros() {
  int n = LEVEL;
  n += BUMP(n) + TWICE(local_value);
  n += BUMP(LEVEL);
  return n + GLUE(local_, value);
}
