#include "h.h"
// an explicit specialization here, an implicit instantiation in b.cpp
template <> int t<int>(int) { return 2; }
int use_a() { return t(1); }
