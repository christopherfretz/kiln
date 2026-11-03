// For ../../missing-include-dirs.cpp: includes its unguarded inner
// header through a macro.
#ifndef MR_M_H
#define MR_M_H
#define MR_INNER "mr-i.h"
#include MR_INNER
#endif
