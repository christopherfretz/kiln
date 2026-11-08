// [basic.start.term] `[[clang::no_destroy]]` keeps a variable with static
// or thread storage duration from registering its destructor, and
// `[[clang::always_destroy]]` keeps it registered (clang
// `VarDecl::isNoDestroy`; spec/landed/sema/WP-805.md). WP-805.
//@ expect: run
//@ strict-diags
//@ stdout: main|~tl_kept|~local_kept|~kept|
extern "C" int printf(const char *, ...);
extern "C" int atexit(void (*)());

// Registered before any destructor below, so it runs last.
struct EndLine {
  EndLine() { atexit([] { printf("\n"); }); }
} end_line;

struct Noisy {
  const char *name;
  ~Noisy() { printf("~%s|", name); }
};

[[clang::no_destroy]] Noisy dropped{"dropped"};
[[clang::always_destroy]] Noisy kept{"kept"};
__attribute__((no_destroy)) static Noisy dropped_gnu{"dropped_gnu"};

void locals() {
  [[clang::no_destroy]] static Noisy local_dropped{"local_dropped"};
  static Noisy local_kept{"local_kept"};
  [[clang::no_destroy]] thread_local Noisy tl_dropped{"tl_dropped"};
  thread_local Noisy tl_kept{"tl_kept"};
  (void)local_dropped.name;
  (void)local_kept.name;
  (void)tl_dropped.name;
  (void)tl_kept.name;
}

int main() {
  locals();
  printf("main|");
  (void)dropped.name;
  (void)dropped_gnu.name;
}
