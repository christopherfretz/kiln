//@ expect: run
//@ stdout: 7 3 5
// WP-726: `Block::get` where `Block` is a member class of the current
// instantiation declared in the class but defined out of line later: the
// specifier is dependent ([temp.dep.type], clang `computeDeclContext`), so
// the name is looked up at instantiation, not in the incomplete class
// (folly QuotientMultiSet).
#include <cstdio>

template <class I> class Q {
public:
  int f() const { return getBlock()->v; }
  int g() const { return Block::k; }
  int h() const { typename Block::type t = 5; return t; }
private:
  struct Block;
  const Block* getBlock() const { return Block::get(data_); }
  const char* data_ = nullptr;
};

template <class I> struct Q<I>::Block {
  using type = int;
  static constexpr int k = 3;
  static const Block* get(const char*) { static Block b{7}; return &b; }
  int v;
};

int main() {
  Q<long> q;
  std::printf("%d %d %d\n", q.f(), q.g(), q.h());
}
