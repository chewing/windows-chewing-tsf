cmake_minimum_required(VERSION 2.8.8)

project(ChewingIme)

if(NOT DEFINED PROJECT_LIBCHEWING)
    message(FATAL_ERROR "PROJECT_LIBCHEWING must be provided")
endif()

# http://www.utf8everywhere.org/
add_definitions(/D_UNICODE=1 /DUNICODE=1)

include_directories(
    ${PROJECT_LIBCHEWING}/include
    ${CMAKE_SOURCE_DIR}
)

add_library(ChewingIme SHARED
    # core IME implementation
    ${PROJECT_SOURCE_DIR}/ChewingIME.cpp
    ${PROJECT_SOURCE_DIR}/ChewingIME.def
    # resources
    # ${PROJECT_SOURCE_DIR}/ChewingIME.rc
    # ${PROJECT_SOURCE_DIR}/mainicon.ico
)
target_link_libraries(ChewingIme
	Imm32.lib
    chewing_static
)
set_target_properties(ChewingIme PROPERTIES
	SUFFIX ".ime"
)
