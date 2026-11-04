//@ expect: run
//@ args: -O2
//@ stdout: id 0
// [basic.indet]/1 (C++26): `Handler h;` gets kiln's erroneous value, all
// zero bytes (spec/codegen.md §7r), and the program then never reads an
// erroneous byte: the only read past `h.ctx` sits under an `if consteval`
// guard ([stmt.if]/4), which a run-time call never takes. This is the
// shape of a formatting library's argument-id check that once failed at
// -O2 under kiln (WP-584; fixed by WP-588): kiln folded the guard to true outside a
// constant context, and the stray read of the zero-filled `buf` made
// argument 0 look out of range. The fill was right; the fold was not.
extern "C" int printf(const char *, ...);
extern "C" [[noreturn]] void exit(int);
constexpr bool is_ce() {
  if consteval {
    return true;
  } else {
    return false;
  }
}
[[noreturn, gnu::noinline]] void fail(const char *m) {
  printf("error %s\n", m);
  exit(1);
}
struct ParseCtx {
  const char *begin;
  int next_id;
  void check_id(int id);
  int next() {
    int id = next_id++;
    check_id(id);
    return id;
  }
};
struct CompileCtx : ParseCtx {
  int num_args;
};
void ParseCtx::check_id(int id) {
  if (is_ce() && sizeof(int) == 4) {
    if (id >= static_cast<CompileCtx *>(this)->num_args)
      fail("argument not found");
  }
}
struct Handler {
  ParseCtx ctx;
  char buf[24];
};
[[gnu::noinline]] int run_handler(const char *fmt) {
  Handler h;
  h.ctx.begin = fmt;
  h.ctx.next_id = 0;
  return h.ctx.next();
}
int main() { printf("id %d\n", run_handler("{:08b}")); }
