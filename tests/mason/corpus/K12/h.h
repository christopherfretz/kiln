// odr-uses an entity with internal linkage: USES_INTERNAL in both units
namespace {
int counter;
}
inline int f(int a) { return counter += a; }
