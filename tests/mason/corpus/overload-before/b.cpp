// another overload declared before the definition
inline int g(int) { return 2; }
#include "h.h"
int (*keep_b)(int) = f;
