#include "h.h"
int use_b() { return g(); }
int (*keep_b)(int) = f;
