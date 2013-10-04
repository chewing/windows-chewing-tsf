cmake_minimum_required(VERSION 2.8.8)

project(ChewingTextService)

if(NOT DEFINED PROJECT_LIBCHEWING)
    message(FATAL_ERROR "PROJECT_LIBCHEWING must be provided")
endif()

# http://www.utf8everywhere.org/
add_definitions(/D_UNICODE=1 /DUNICODE=1)

include_directories(
    ${PROJECT_LIBCHEWING}/include
    ${CMAKE_SOURCE_DIR}
)

#------------------------------------------------------------------------------
# TSF implementation
add_library(ChewingTextService SHARED
    # core IME implementation
    ${PROJECT_SOURCE_DIR}/ChewingImeModule.cpp
    ${PROJECT_SOURCE_DIR}/ChewingImeModule.h
    ${PROJECT_SOURCE_DIR}/ChewingTextService.cpp
    ${PROJECT_SOURCE_DIR}/ChewingTextService.def
    ${PROJECT_SOURCE_DIR}/ChewingTextService.h
    ${PROJECT_SOURCE_DIR}/DllEntry.cpp
    ${PROJECT_SOURCE_DIR}/ChewingConfig.cpp
    ${PROJECT_SOURCE_DIR}/ChewingConfig.h
    # resources
    ${PROJECT_SOURCE_DIR}/ChewingTextService.rc
    ${PROJECT_SOURCE_DIR}/mainicon.ico
)
set_target_properties(ChewingTextService PROPERTIES
    COMPILE_DEFINITIONS "USE_TSF=1"
)
target_link_libraries(ChewingTextService
    chewing_static
    libIME_static
)

#------------------------------------------------------------------------------
# IME implementation
add_library(ChewingIME SHARED
    # core Imm32 implementation
    ${PROJECT_SOURCE_DIR}/ChewingIME.cpp
    ${PROJECT_SOURCE_DIR}/ChewingIME.def
    # core IME implementation
    ${PROJECT_SOURCE_DIR}/ChewingImeModule.cpp
    ${PROJECT_SOURCE_DIR}/ChewingImeModule.h
    ${PROJECT_SOURCE_DIR}/ChewingTextService.cpp
    ${PROJECT_SOURCE_DIR}/ChewingTextService.h
    ${PROJECT_SOURCE_DIR}/DllEntry.cpp
    ${PROJECT_SOURCE_DIR}/ChewingConfig.cpp
    ${PROJECT_SOURCE_DIR}/ChewingConfig.h
    # resources
    ${PROJECT_SOURCE_DIR}/ChewingTextService.rc
    ${PROJECT_SOURCE_DIR}/mainicon.ico
)
target_link_libraries(ChewingIME
    Imm32.lib
    chewing_static
    libIME_static
)
set_target_properties(ChewingIME PROPERTIES
    SUFFIX ".ime"
    COMPILE_DEFINITIONS "USE_IMM32=1"
)
