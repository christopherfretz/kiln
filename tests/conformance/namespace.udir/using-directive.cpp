//@ expect: compile-ok
namespace lib {
int count = 3;
int twice(int n) { return n * 2; }
}
using namespace lib;
int use() { return twice(count); }
