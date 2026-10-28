//@ expect: pp
//@ args: -I%{test_dir}/Inputs
// __FILE__, __FILE_NAME__ and __INCLUDE_LEVEL__ in the main file, in an
// included file, after #line, and when used in a macro defined in a
// header but expanded in the main file.
__FILE__ __FILE_NAME__ __INCLUDE_LEVEL__
#include "Inputs/file-macros.h"
WHERE
#line 50 "renamed/main.cpp"
__FILE__ __FILE_NAME__ __LINE__ WHERE
