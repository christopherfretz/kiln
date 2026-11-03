// [expr.prim.lambda.capture]/6, libc++-style move capture: `[p(std::move(q))]`
// direct-initializes a move-only closure member; the closure is itself
// move-only and the source is left empty.
//@ expect: run
//@ stdout: 42 1 43 1
#include <cstdio>
#include <memory>
#include <utility>
int main() {
  auto up = std::make_unique<int>(42);
  auto o = [p(std::move(up))] { return *p; };
  std::printf("%d %d ", o(), up == nullptr);
  auto o2 = std::move(o);
  auto up2 = std::make_unique<int>(43);
  auto b = [p{std::move(up2)}]() mutable { return std::move(p); };
  auto got = b();
  std::printf("%d %d\n", *got, o2() == 42);
}
