struct MainPacked { char c; long l; };
#pragma pack(pop)
struct MainAfter { char c; long l; };
static_assert(sizeof(MainPacked) == 1 + sizeof(long));
static_assert(sizeof(MainAfter) == 2 * sizeof(long));
int main() { return sizeof(PfxBefore) + sizeof(PfxInside) + sizeof(MainPacked) + sizeof(MainAfter); }
