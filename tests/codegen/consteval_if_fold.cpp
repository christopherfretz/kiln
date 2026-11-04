// [stmt.if]/4 with [expr.const]/20: the condition of a run-time `if`
// is not manifestly constant-evaluated, so when codegen folds it (clang
// `ConstantFoldsToSimpleInteger`) an `if consteval` reached through a
// constexpr call takes its second branch: `is_ce()` folds to false and
// the guarded block is dropped, `not_ce()` to true. The shape is a
// formatting library's argument-id check, which casts the parse context
// to a derived compile-time context and reads past the run-time object
// only under the constant-evaluation guard; kiln used to fold the guard
// to true, emitting that read, which hit the zero-filled bytes of the
// enclosing C++26 erroneous-value initialization (spec/codegen.md §7r,
// WP-584). The variables also get clang's zero fill (`args`).
//@ args: -ftrivial-auto-var-init=zero
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
