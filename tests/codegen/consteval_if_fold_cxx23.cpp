// consteval_if_fold.cpp under -std=c++23 with an explicit
// -ftrivial-auto-var-init=zero ([stmt.if]/4, [expr.const]/20): the
// folded run-time `if` drops the `if consteval`-guarded block in C++23
// too, and the variables get the requested zero fill (WP-584).
//@ args: -std=c++23 -ftrivial-auto-var-init=zero
constexpr bool is_ce() {
  if consteval {
    return true;
  } else {
    return false;
  }
}
constexpr bool not_ce() {
  if !consteval {
    return true;
  } else {
    return false;
  }
}
[[noreturn]] void fail(const char *);
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
void use(void *);
int run_handler(const char *fmt) {
  Handler h;
  h.ctx.begin = fmt;
  h.ctx.next_id = 0;
  use(&h);
  return h.ctx.next();
}
int not_ce_guard(int x) {
  int r;
  use(&r);
  if (not_ce())
    r = x;
  return r;
}
