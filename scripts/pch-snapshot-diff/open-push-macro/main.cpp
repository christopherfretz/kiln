int inner = PFX_LEVEL;
#pragma pop_macro("PFX_LEVEL")
int outer = PFX_LEVEL;
#pragma pop_macro("PFX_LEVEL")
int after = PFX_LEVEL;
int main() { return inner * 10 + outer - 21; }
