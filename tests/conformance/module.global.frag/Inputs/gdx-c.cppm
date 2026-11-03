// The importing module of gmf-dedup-import-gmf.cpp (the std.compat shape):
// the same GMF declarations as gdx-m.cppm, then `export import gdx_m;`.
module;
namespace std { enum class byte : unsigned char; }
namespace std { template <class T> inline const bool is_b = false; template <> inline const bool is_b<byte> = true; }
namespace std { enum class byte : unsigned char {}; }
export module gdx_c;
export import gdx_m;
