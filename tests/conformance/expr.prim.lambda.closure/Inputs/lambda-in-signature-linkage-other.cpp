// The second unit of ../lambda-in-signature-linkage-two-units-run.cpp:
// the same specializations, their addresses and their closures' statics.
#include "lambda-in-signature-linkage.h"
#define ADDR(e) (void *)(e),
void *other_statics[] = {USES(ADDR)};
void (*other_ret)() = reinterpret_cast<void (*)()>(&ret<int>);
void (*other_nret)() = reinterpret_cast<void (*)()>(&S<int>::nret);
