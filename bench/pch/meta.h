// The pch group's `meta` header set (spec/bench.md "The pch group"): GCC
// 16's <meta> (C++26 static reflection) with the containers a reflection
// program typically uses. Compiled with `--gcc-toolchain=` naming a GCC
// 16 installation: the libstdc++ of GCC 14 has no <meta>.
#include <meta>
#include <algorithm>
#include <array>
#include <map>
#include <string>
#include <string_view>
#include <vector>
