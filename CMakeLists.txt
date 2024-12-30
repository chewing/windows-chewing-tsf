cmake_minimum_required(VERSION 3.24.0)

project(windows-chewing-tsf LANGUAGES CXX)

add_subdirectory(libchewing)

set(CPACK_PACKAGE_CHECKSUM SHA256)
set(CPACK_PACKAGE_VERSION 24.10.1)
set(CPACK_SOURCE_IGNORE_FILES "/build" "/target" "/dist" "/\\\\..*")
set(CPACK_SOURCE_GENERATOR ZIP)
include(CPack)
