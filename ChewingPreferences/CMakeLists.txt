cmake_minimum_required(VERSION 3.24.0)
project(ChewingPreferences LANGUAGES CXX)

# http://www.utf8everywhere.org/
add_definitions(
	/D_UNICODE=1 /DUNICODE=1 # do Unicode build
	/D_CRT_SECURE_NO_WARNINGS # disable warnings about old libc functions
)
if (MSVC)
	add_compile_options(/utf-8)
endif()

# Static link MSVC runtime
set(CMAKE_CXX_STANDARD 17)
set(CMAKE_MSVC_RUNTIME_LIBRARY "MultiThreaded")

add_executable(ChewingPreferences WIN32
    # Entry point
    ${PROJECT_SOURCE_DIR}/ChewingPreferences.cpp
    # Configurations
    ../chewing_tip/ChewingConfig.cpp
    ../chewing_tip/ChewingConfig.h
    # Configuration dialog
    ${PROJECT_SOURCE_DIR}/Dialog.cpp
    ${PROJECT_SOURCE_DIR}/Dialog.h
    ${PROJECT_SOURCE_DIR}/PropertyDialog.cpp
    ${PROJECT_SOURCE_DIR}/PropertyDialog.h
    ${PROJECT_SOURCE_DIR}/PropertyPage.cpp
    ${PROJECT_SOURCE_DIR}/PropertyPage.h
    ${PROJECT_SOURCE_DIR}/TypingPropertyPage.cpp
    ${PROJECT_SOURCE_DIR}/TypingPropertyPage.h
    ${PROJECT_SOURCE_DIR}/UiPropertyPage.cpp
    ${PROJECT_SOURCE_DIR}/UiPropertyPage.h
    ${PROJECT_SOURCE_DIR}/KeyboardPropertyPage.cpp
    ${PROJECT_SOURCE_DIR}/KeyboardPropertyPage.h
    ${PROJECT_SOURCE_DIR}/SymbolsPropertyPage.cpp
    ${PROJECT_SOURCE_DIR}/SymbolsPropertyPage.h
    # About dialog
    ${PROJECT_SOURCE_DIR}/AboutDialog.cpp
    ${PROJECT_SOURCE_DIR}/AboutDialog.h
    # resources
    ${PROJECT_SOURCE_DIR}/ChewingPreferences.rc
	${PROJECT_SOURCE_DIR}/ChewingPreferences.exe.manifest
    ../chewing_tip/mainicon.ico
    ../chewing_tip/logo.bmp
)
target_include_directories(ChewingPreferences
    PRIVATE ../chewing_tip
)
target_link_libraries(ChewingPreferences
	comctl32.lib
)
