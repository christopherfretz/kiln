#include "h.h"
int use_a() { return g(); }
int (*keep_a)(int) = f;
