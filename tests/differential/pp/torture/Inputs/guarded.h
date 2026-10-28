// A comment before the guard does not defeat the multiple-include
// optimization.
#ifndef TORTURE_GUARDED_H
#define TORTURE_GUARDED_H
guarded_h_body
#include "guarded.h"
#endif
