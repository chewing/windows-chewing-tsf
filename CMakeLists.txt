cmake_minimum_required(VERSION 3.24.0)

project(windows-chewing-tsf LANGUAGES CXX)

option(BUILD_SHARED_LIBS "Build using shared libraries" ON)

# http://www.utf8everywhere.org/
add_definitions(
	/D_UNICODE=1 /DUNICODE=1 # do Unicode build
	/D_CRT_SECURE_NO_WARNINGS # disable warnings about old libc functions
)
set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CXX_EXTENSIONS OFF)

set(CMAKE_C_FLAGS "${CMAKE_C_FLAGS} /utf-8") # set source code encoding to UTF-8
set(CMAKE_CXX_FLAGS "${CMAKE_CXX_FLAGS} /GR-") # turn off C++ RTTI
set(CMAKE_CXX_FLAGS "${CMAKE_CXX_FLAGS} /utf-8") # set source code encoding to UTF-8
set(CMAKE_C_FLAGS_RELEASE "${CMAKE_C_FLAGS_RELEASE} /GL /Gw")
set(CMAKE_CXX_FLAGS_RELEASE "${CMAKE_CXX_FLAGS_RELEASE} /GL /Gw")
set(CMAKE_MSVC_RUNTIME_LIBRARY "MultiThreadedDLL")
set(CMAKE_INTERPROCEDURAL_OPTIMIZATION $<CONFIG:Release>)

enable_testing()

set(PROJECT_LIBCHEWING ${PROJECT_SOURCE_DIR}/libchewing)
set(CHEWING_DATA_PREFIX ${PROJECT_BINARY_DIR}/libchewing/data)

add_subdirectory(${PROJECT_LIBCHEWING})

add_subdirectory(${PROJECT_SOURCE_DIR}/libIME)

add_subdirectory(${PROJECT_SOURCE_DIR}/ChewingTextService)

add_subdirectory(${PROJECT_SOURCE_DIR}/ChewingPreferences)
