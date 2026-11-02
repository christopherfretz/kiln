// Module mr.b for module.reach/missing-import.cpp: global module
// fragment declarations, module-linkage declarations and imports that
// are not re-exported.
module;
namespace gn {
int gmf_qual();
}
int gmf_fn();
struct GmfType {};
export module mr.b;
import mr.a;
import :part;
export int b_fn() { return a_fn() + part_fn(); }
export namespace en {
int exported_fn() { return 1; }
}
namespace en {
int linkage_fn() { return 2; }
struct LinkageType {};
} // namespace en
int linkage_global() { return 4; }
int linkage_other() { return 5; }
struct LinkageGlobalType {};
