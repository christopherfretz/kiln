// [stmt.ranged]/1.3: the AST of range-based for over a class with member
// `begin`/`end` and over a braced-init-list (std::initializer_list).
//@ expect: ast
//@ ast-filter: sum_class
#include <initializer_list>

struct R {
  int d[2];
  int* begin();
  int* end();
};

int sum_class(R r) {
  int s = 0;
  for (auto&& x : r) s += x;
  for (int x : {1, 2}) s += x;
  return s;
}
