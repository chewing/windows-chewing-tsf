fn main() {
    embedinator::ResourceBuilder::from_env()
        .add_string("ProductName", "Chewing TSF Input Method")
        .add_string(
            "FileDescription",
            "TSF registration helper for the Chewing IME",
        )
        .add_string("CompanyName", "Chewing IME Project")
        .add_string(
            "LegalCopyright",
            "Copyright (C) 2026 Kan-Ru Chen. Licensed GPL-3.0-or-later.",
        )
        .add_string("OriginalFilename", "tsfreg.exe")
        .add_string(
            "Comments",
            "https://codeberg.org/chewing/windows-chewing-tsf",
        )
        .finish();
}
