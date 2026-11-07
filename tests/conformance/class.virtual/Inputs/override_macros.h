// A system header (found through -isystem) whose macros declare methods,
// as a mocking library's do.
#define DECLARE_METHOD(ret, name, ...) virtual ret name(int) __VA_ARGS__
#define NAME_OF(name) name
