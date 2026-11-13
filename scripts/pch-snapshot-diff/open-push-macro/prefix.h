// Ends with a #pragma push_macro whose pop is in the main file.
#define PFX_LEVEL 1
#pragma push_macro("PFX_LEVEL")
#undef PFX_LEVEL
#define PFX_LEVEL 2
