// [support.srcloc.cons], [dcl.fct.default]/9: at run time the location
// builtins give the same values as in constant evaluation; a default argument
// takes the call site's location, and __builtin_source_location() points at
// a static object per location.
//@ expect: run
//@ stdout: 24 35 35
//@ stdout: 20
//@ stdout: 30
//@ stdout: main main
//@ stdout: int main() 39 19
//@ stdout: [] 32 18 []
//@ stdout: 0
//@ stdout: 1
//@ stdout: 1
//@ stdout: g|void N::g(int, char *)
//@ stdout: builtins-srcloc-run.cpp 1
namespace std {
struct source_location {
  struct __impl { const char *_M_file_name; const char *_M_function_name; unsigned _M_line; unsigned _M_column; };
};
} // namespace std
using impl = std::source_location::__impl;
extern "C" int printf(const char *, ...);
unsigned col(unsigned c = __builtin_COLUMN()) { return c; }
unsigned lin(unsigned c = __builtin_LINE()) { return c; }
unsigned lin2(unsigned c = lin()) { return c; }
const char *fn(const char *c = __builtin_FUNCTION()) { return c; }
const impl *sl(const impl *p = __builtin_source_location()) { return p; }
namespace N {
void g(int, char *) { printf("%s|%s\n", fn(), sl()->_M_function_name); }
} // namespace N
const impl *gp = __builtin_source_location();
const char *gf = __builtin_FUNCTION();
int main() {
  printf("%u %u %u\n", col(), lin(), lin2());
  printf("%u\n",   (col)());
  printf("%u\n", (unsigned)  col  ( ) );
  printf("%s %s\n", fn(), __builtin_FUNCTION());
  const impl *p = sl();
  printf("%s %u %u\n", p->_M_function_name, p->_M_line, p->_M_column);
  printf("[%s] %u %u [%s]\n", gp->_M_function_name, gp->_M_line, gp->_M_column, gf);
  printf("%d\n", sl() == sl());
  const impl *first = nullptr;
  for (int i = 0; i < 2; ++i) {
    const impl *q = sl();
    if (!first) first = q;
    printf("%d\n", q == first);
  }
  N::g(0, nullptr);
  printf("%s %d\n", __builtin_FILE_NAME(), p->_M_file_name == __builtin_FILE());
}
