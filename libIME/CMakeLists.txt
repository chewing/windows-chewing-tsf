find_package(Corrosion QUIET)
if(NOT Corrosion_FOUND)
    FetchContent_Declare(
        Corrosion
        GIT_REPOSITORY https://github.com/corrosion-rs/corrosion.git
        GIT_TAG 64289b1d79d6d19cd2e241db515381a086bb8407 # v0.5
        FIND_PACKAGE_ARGS
    )
    FetchContent_MakeAvailable(Corrosion)
endif()

corrosion_import_crate(MANIFEST_PATH Cargo.toml CRATES libime2 CRATE_TYPES staticlib)
corrosion_add_target_rustflags(libime2 "-C target-feature=+crt-static")

if(CMAKE_CXX_COMPILER_ID MATCHES "Clang" AND CMAKE_CXX_SIMULATE_ID MATCHES "MSVC")
    corrosion_set_env_vars(libime2 "CFLAGS=-EHsc" "CXXFLAGS=-EHsc")
endif()

find_program(MIDL midl)
add_custom_command(
    OUTPUT
        dlldata.c
        libime2.h
        libime2_i.c
        libime2_p.c
    COMMAND MIDL ${CMAKE_CURRENT_SOURCE_DIR}/idl/libime2.idl
    MAIN_DEPENDENCY idl/libime2.idl
)

add_library(libIME_static STATIC
    # Core TSF part
    libIME.cpp
    libIME.h
    TextService.cpp
    TextService.h
    KeyEvent.cpp
    KeyEvent.h
    EditSession.cpp
    EditSession.h
    LangBarButton.cpp
    LangBarButton.h
    Utils.cpp
    Utils.h
    # Rust interop
    dlldata.c
    libime2.h
    libime2_i.c
    libime2_p.c
)

target_include_directories(libIME_static PUBLIC ${CMAKE_CURRENT_BINARY_DIR})

target_link_libraries(libIME_static
    PUBLIC libime2
    PUBLIC shlwapi.lib
    PUBLIC d2d1.lib
    PUBLIC d3d11.lib
    PUBLIC dwrite.lib
    PUBLIC dcomp.lib

    PUBLIC Propsys.lib
    PUBLIC RuntimeObject.lib
)