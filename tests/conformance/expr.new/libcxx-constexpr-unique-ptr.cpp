// [unique.ptr], [expr.const]/5.20: libc++'s std::unique_ptr (constexpr
// since C++23) keeps its pointer in a __compressed_pair whose empty
// default_delete base shares the pointer's address; the pointer survives
// the deleter's value-initialization ([intro.object]/9).
//@ expect: compile-ok
//@ args: -stdlib=libc++
#include <memory>
#include <utility>

constexpr int single() {
  std::unique_ptr<int> p(new int(4));
  *p += 1;
  return *p;
}
static_assert(single() == 5);

constexpr int array() {
  std::unique_ptr<int[]> p(new int[3]{1, 2, 3});
  p[1] = 7;
  return p[1] + p.get()[2];
}
static_assert(array() == 10);

constexpr int moved() {
  auto p = std::make_unique<int[]>(4);
  p[3] = 9;
  std::unique_ptr<int[]> q = std::move(p);
  int *raw = q.release();
  int r = raw[3] + (p ? 1 : 0);
  delete[] raw;
  return r;
}
static_assert(moved() == 9);

constexpr int reset() {
  std::unique_ptr<int> p = std::make_unique<int>(1);
  p.reset(new int(2));
  return *p;
}
static_assert(reset() == 2);
