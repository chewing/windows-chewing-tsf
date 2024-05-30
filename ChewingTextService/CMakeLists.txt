project(ChewingTextService)

if(NOT DEFINED PROJECT_LIBCHEWING)
    message(FATAL_ERROR "PROJECT_LIBCHEWING must be provided")
endif()

include_directories(
    ${PROJECT_LIBCHEWING}/include
    ${CMAKE_SOURCE_DIR}
)

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

target_link_libraries(ChewingTextService
    libchewing
    libIME_static
)
