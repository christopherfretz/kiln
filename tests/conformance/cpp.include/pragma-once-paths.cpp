// [cpp.include], [cpp.pragma], [cpp.predefined]: a file is one file however
// its path is spelled, so '#pragma once' skips it through a '..' path, and
// __FILE__ and the line markers keep the spelling it was first entered by
// (clang). (Symlinked spellings: kiln_driver's depfiles test
// every_path_a_header_is_reached_by_is_a_dependency.)
//@ expect: pp
//@ pp-linemarkers
#include "Inputs/search/paths/once.h"
#include "Inputs/search/next/../paths/once.h"
#include "Inputs/search/paths/../paths/plain.h"
#include "Inputs/search/paths/plain.h"
