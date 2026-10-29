// Globals with constant initializers: scalars, arrays, structs, strings
// and pointers into other globals.
struct Pt { int x, y; };
int g_int = 42;
long g_long = -1;
double g_double = 2.5;
const char* g_str = "global";
char g_chars[] = "chars";
int g_arr[5] = {1, 2, 3};
Pt g_pt = {3, 4};
Pt g_pts[2] = {{1, 2}, {3, 4}};
int* g_ptr = &g_arr[2];
static int s_hidden = 7;
const int k_const = 9;
int read_globals() { return g_int + (int)g_long + (int)g_double + g_str[0] + g_chars[1] + g_arr[2] + g_pt.y + g_pts[1].x + *g_ptr + s_hidden + k_const; }
