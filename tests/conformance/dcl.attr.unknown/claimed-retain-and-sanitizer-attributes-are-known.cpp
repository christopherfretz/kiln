//@ expect: compile-ok
//@ std: c++20
//@ strict-diags
//@ xfail(kiln): WP-789 follow-up, gnu::retain and disable_sanitizer_instrumentation are claimed by __has_cpp_attribute/__has_attribute but sema warns "unknown attribute ... ignored" (seen building folly::coro: FOLLY_KEEP, Subprocess.cpp)
// clang knows both attributes, and kiln's feature table says so: a
// library that tests for them and then uses them gets no warning.
#if __has_cpp_attribute(gnu::retain)
[[gnu::used, gnu::retain]] void kept() {}
#endif

#if __has_attribute(disable_sanitizer_instrumentation)
__attribute__((noinline, disable_sanitizer_instrumentation)) int raw() {
  return 1;
}
#endif

int main() {}
