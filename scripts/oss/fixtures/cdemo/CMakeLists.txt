# The c_demo row (WP-1277): a C library, a C program and a C++ program over
# it, so a `c = "kiln"` build logs C and C++ TUs side by side.
cmake_minimum_required(VERSION 3.20)
project(cdemo C CXX)
set(CMAKE_C_STANDARD 11)
add_library(sum STATIC sum.c)
add_executable(capp capp.c)
target_link_libraries(capp sum)
add_executable(cxxapp cxxapp.cpp)
target_link_libraries(cxxapp sum)
