// [class.derived.general]/2, [basic.lookup.general]/4: the class-name of
// a base-specifier is found by type-only lookup, so a function, function
// template or variable of the same scope that hides the class from
// ordinary lookup ([basic.scope.hiding]) does not hide it here (abseil
// btree_test: TEST_F(BtreeMapTest, ...) next to a function template
// BtreeMapTest).
//@ expect: run
//@ stdout: 3 4 5
#include <cstdio>
namespace {
template <class T> void Fix() {}
}
namespace {
class Fix {
public:
  int v = 3;
};
}
void Other() {}
struct Other {
  int v = 4;
};
struct Var {
  int v = 5;
};
int Var = 0;
struct D : public Fix {};
struct E : Other {};
namespace n {
struct F : ::Var {};
}
int main() { std::printf("%d %d %d\n", D().v, E().v, n::F().v); }
