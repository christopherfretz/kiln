// The imported module of gmf-dedup-import-gmf.cpp: its global module
// fragment declares, uses and defines the same `std::byte` and variable
// template as the importer's (libc++'s std.cppm and std.compat.cppm both
// include <cstddef> and the type-trait headers in their GMFs).
module;
namespace std { enum class byte : unsigned char; }
namespace std { template <class T> inline const bool is_b = false; template <> inline const bool is_b<byte> = true; }
namespace std { enum class byte : unsigned char {}; }
export module gdx_m;
export namespace std { using std::byte; using std::is_b; }
