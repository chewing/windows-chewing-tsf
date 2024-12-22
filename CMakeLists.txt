cmake_minimum_required(VERSION 3.24.0)

project(windows-chewing-tsf LANGUAGES CXX)

option(BUILD_SHARED_LIBS "Build using shared libraries" OFF)

# http://www.utf8everywhere.org/
add_definitions(
	/D_UNICODE=1 /DUNICODE=1 # do Unicode build
	/D_CRT_SECURE_NO_WARNINGS # disable warnings about old libc functions
)
set(CMAKE_CXX_STANDARD 17)

if (MSVC)
	add_compile_options(/utf-8)
endif()

# Static link MSVC runtime
set(CMAKE_MSVC_RUNTIME_LIBRARY "MultiThreaded")

add_subdirectory(libchewing)
add_subdirectory(libIME)
add_subdirectory(ChewingTextService)
add_subdirectory(ChewingPreferences)

set(CPACK_PACKAGE_CHECKSUM SHA256)
set(CPACK_PACKAGE_VERSION 24.10.1)
set(CPACK_SOURCE_IGNORE_FILES "/build" "/target" "/dist" "/\\\\..*")
set(CPACK_SOURCE_GENERATOR ZIP)
include(CPack)
