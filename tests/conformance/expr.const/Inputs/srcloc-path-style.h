// Reached through a messy -I by builtins-srcloc-path-style*.cpp: the file
// spelling of the source-location builtins follows
// -fkiln-source-path-style= (clang's as written by default).
constexpr const char *h_file = __builtin_FILE();
constexpr const char *h_file_name = __builtin_FILE_NAME();
constexpr const char *h_current = std::source_location::current().file_name();
constexpr const char *h_where = where();
constexpr const char *h_macro = __FILE__;
