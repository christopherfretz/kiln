// A header imported as a header unit by module.import/header-unit*.cpp.
#ifndef KILN_TEST_HU_H
#define KILN_TEST_HU_H
#define HU_VALUE 40
#define HU_ADD(a, b) ((a) + (b))
inline int hu_twice(int x) { return 2 * x; }
struct HuPoint {
  int x, y;
};
#endif
