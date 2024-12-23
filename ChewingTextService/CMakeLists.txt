project(ChewingTextService)

include_directories(
    ${CMAKE_SOURCE_DIR}
)

add_library(ChewingTextService SHARED
    # core IME implementation
    ${PROJECT_SOURCE_DIR}/ChewingTextService.cpp
    ${PROJECT_SOURCE_DIR}/ChewingTextService.def
    ${PROJECT_SOURCE_DIR}/ChewingTextService.h
    ${PROJECT_SOURCE_DIR}/DllEntry.cpp
    ${PROJECT_SOURCE_DIR}/CClassFactory.cpp
    ${PROJECT_SOURCE_DIR}/CClassFactory.h
    ${PROJECT_SOURCE_DIR}/ChewingConfig.cpp
    ${PROJECT_SOURCE_DIR}/ChewingConfig.h
    ${PROJECT_SOURCE_DIR}/Utils.cpp
    ${PROJECT_SOURCE_DIR}/Utils.h
    # resources
    ${PROJECT_SOURCE_DIR}/ChewingTextService.rc
    ${PROJECT_SOURCE_DIR}/mainicon.ico
    # manifest
    ${PROJECT_SOURCE_DIR}/ChewingTextService.manifest
)

target_link_libraries(ChewingTextService
    libchewing
    libIME_static
)
target_link_options(ChewingTextService
    PRIVATE /NODEFAULTLIB:MSVCRT
    PRIVATE /NODEFAULTLIB:MSVCRTD
    PRIVATE /NODEFAULTLIB:LIBCMTD
)
