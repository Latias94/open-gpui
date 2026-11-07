use open_gpui_util::parse_os_release;

#[test]
fn parse_os_release_matches_official_public_contract() {
    let parse: fn(&str) -> Option<String> = parse_os_release;

    for (input, expected) in [
        (
            "NAME=\"Ubuntu\"\nID=ubuntu\nVERSION_ID=\"24.04\"\nPRETTY_NAME=\"Ubuntu 24.04 LTS\"\n",
            Some("ubuntu 24.04"),
        ),
        ("ID=\"debian\"\nVERSION_ID=\"13\"\n", Some("debian 13")),
        ("VERSION_ID=42\nID=fedora\n", Some("fedora 42")),
        ("ID=arch\n", Some("arch")),
        ("VERSION_ID=1\n", None),
        ("", None),
        ("ID='debian'\nVERSION_ID='13'\n", Some("'debian' '13'")),
        ("ID=\"\"arch\"\"\n", Some("arch")),
        ("ID_LIKE=debian\n ID=ubuntu\nVERSION_ID=24.04\n", None),
        ("ID= arch \n", Some(" arch ")),
        ("ID=\nVERSION_ID=\"\"\n", Some(" ")),
    ] {
        let actual: Option<String> = parse(input);
        assert_eq!(actual.as_deref(), expected, "input: {input:?}");
    }
}
