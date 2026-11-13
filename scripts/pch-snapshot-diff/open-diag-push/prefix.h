// Ends inside a #pragma clang diagnostic push; the pop is in the main
// file. A GCC diagnostic mapping in force at the end is carried too.
#pragma GCC diagnostic warning "-Wshadow"
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wunused-variable"
#pragma clang diagnostic error "-Wunused-value"
