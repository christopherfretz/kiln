// [support.srcloc.cons]: the source-location builtins in an included
// header's function bodies give the header's location and function; in
// a default argument they give the call's. With `--pch-diff`, the
// header's snapshot carries the builtin calls and the importer
// computes their values again (WP-1310).
//@ expect: run
//@ std: c++20
#include "Inputs/source_loc.h"

#include <cstring>

int main() {
  if (sl::line_here() != 7 || sl::column_here() != 43)
    return 1;
  if (std::strcmp(sl::func_here(), "func_here") != 0 || std::strcmp(sl::tfunc<int>(), "tfunc") != 0)
    return 2;
  if (sl::caller_line() != 17)
    return 3;
  std::source_location s = std::source_location::current();
  if (sl::tline() != 20 || s.line() != 19 || std::strcmp(s.function_name(), "int main()") != 0)
    return 4;
  std::source_location t = sl::at(std::source_location::current());
  if (t.line() != 22 || std::strstr(t.file_name(), "pch-source-loc-run.cpp") == nullptr)
    return 5;
  if (sl::use_line() != 14 || sl::member_line() != 18)
    return 6;
  return std::strcmp(t.function_name(), "int main()") == 0 ? 0 : 7;
}
