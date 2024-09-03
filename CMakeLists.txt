cmake_minimum_required(VERSION 3.24.0)

project(windows-chewing-tsf LANGUAGES CXX)

option(BUILD_SHARED_LIBS "Build using shared libraries" ON)

# http://www.utf8everywhere.org/
add_definitions(
	/D_UNICODE=1 /DUNICODE=1 # do Unicode build
	/D_CRT_SECURE_NO_WARNINGS # disable warnings about old libc functions
)
set(CMAKE_CXX_STANDARD 20)

if (MSVC)
	add_compile_options(/utf-8)
endif()

# Static link MSVC runtime
set(CMAKE_MSVC_RUNTIME_LIBRARY "MultiThreaded")

add_subdirectory(libchewing)
add_subdirectory(libIME)
add_subdirectory(ChewingTextService)
add_subdirectory(ChewingPreferences)
