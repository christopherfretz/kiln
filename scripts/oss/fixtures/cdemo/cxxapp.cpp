#include <cstdio>
#include <vector>
#include "sum.h"

int main() {
  std::vector<point> pts{{4, 5}, {2, 3}};
  std::printf("c++ %d\n", sum_points(pts.data(), static_cast<int>(pts.size())));
}
