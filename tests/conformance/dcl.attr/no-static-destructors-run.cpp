// [basic.start.term] `-fno-c++-static-destructors` registers no destructor
// for a variable with static or thread storage duration, except one marked
// `[[clang::always_destroy]]` (clang `VarDecl::isNoDestroy`;
// spec/landed/sema/WP-805.md). WP-805.
//@ expect: run
//@ strict-diags
//@ args: -fno-c++-static-destructors
//@ stdout: main|~local_kept|~kept|
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

Noisy dropped{"dropped"};
[[clang::always_destroy]] Noisy kept{"kept"};

void locals() {
  static Noisy local_dropped{"local_dropped"};
  [[clang::always_destroy]] static Noisy local_kept{"local_kept"};
  thread_local Noisy tl_dropped{"tl_dropped"};
  (void)local_dropped.name;
  (void)local_kept.name;
  (void)tl_dropped.name;
}

int main() {
  locals();
  printf("main|");
  (void)dropped.name;
}
