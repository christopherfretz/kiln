//@ expect: run
//@ args: -pthread
//@ stdout: 3 4 13 14 / 1 2 11 12
// GCC "Common Variable Attributes", tls_model: the model selects how a
// thread-local variable is addressed (`AttrDocs` "tls_model"); each
// thread still has its own instance.
#include <cstdio>
#include <thread>

static thread_local int ie __attribute__((tls_model("initial-exec"))) = 1;
static __thread int le __attribute__((tls_model("local-exec"))) = 2;
thread_local int ld [[gnu::tls_model("local-dynamic")]] = 11;
thread_local int gd [[gnu::tls_model("global-dynamic")]] = 12;

int main() {
  std::thread t([] { ie += 2, le += 2, ld += 2, gd += 2; std::printf("%d %d %d %d / ", ie, le, ld, gd); });
  t.join();
  std::printf("%d %d %d %d\n", ie, le, ld, gd);
}
