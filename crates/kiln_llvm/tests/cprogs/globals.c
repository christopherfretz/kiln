// Globals: scalars, arrays, strings, pointers into globals, statics.
int counter = 3;
const char *greeting = "hi\n";
int table[4] = {1, 2, 3};
int *second = &table[1];
static double ratio = 0.5;
extern int ext;
double get(void) { return ratio + ext + counter; }
