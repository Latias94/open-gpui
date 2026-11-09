use super::username_and_password;
use crate::util::windows_credentials_target_name;
use std::ptr;
use windows::{
    Win32::{
        Foundation::ERROR_NOT_FOUND,
        Security::Credentials::{
            CRED_PERSIST_SESSION, CRED_TYPE_GENERIC, CREDENTIALW, CredDeleteW, CredFree, CredReadW,
            CredWriteW,
        },
    },
    core::{PCWSTR, PWSTR},
};

#[test]
fn test_read_credential_with_username() {
    let (username, password) = round_trip_credential(Some("alice"), b"secret");
    assert!(username == "alice", "the account must round trip");
    assert!(password == b"secret", "the password must round trip");
}

#[test]
fn test_read_credential_without_username() {
    let (username, password) = round_trip_credential(None, b"secret");
    assert!(username.is_empty(), "a null account must decode as empty");
    assert!(password == b"secret", "the password must round trip");
}

#[test]
fn credential_manager_round_trips_empty_and_binary_blobs() {
    for secret in [&[][..], &[0, 0xff, 0x61, 0][..]] {
        let (username, password) = round_trip_credential(Some("账户🦀"), secret);
        assert!(username == "账户🦀", "Unicode accounts must round trip");
        assert!(password == secret, "all credential bytes must round trip");
    }
}

struct OwnedCredentialTarget(Vec<u16>);

impl Drop for OwnedCredentialTarget {
    fn drop(&mut self) {
        // SAFETY: This terminated target is unique to this test and remains alive during the call.
        let result =
            unsafe { CredDeleteW(PCWSTR::from_raw(self.0.as_ptr()), CRED_TYPE_GENERIC, None) };
        if result.is_err_and(|error| error.code() != ERROR_NOT_FOUND.to_hresult()) {
            eprintln!("failed to remove the owned credential test item");
        }
    }
}

fn round_trip_credential(username: Option<&str>, secret: &[u8]) -> (String, Vec<u8>) {
    let target = windows_credentials_target_name(&format!(
        "open-gpui-test://credential-copy/{}",
        uuid::Uuid::new_v4()
    ));
    let mut target = OwnedCredentialTarget(target.encode_utf16().chain(Some(0)).collect());
    let mut username = username
        .map(|value| value.encode_utf16().chain(Some(0)).collect::<Vec<_>>())
        .unwrap_or_default();
    let mut secret = secret.to_vec();
    let credential = CREDENTIALW {
        Type: CRED_TYPE_GENERIC,
        TargetName: PWSTR::from_raw(target.0.as_mut_ptr()),
        CredentialBlobSize: secret.len() as u32,
        CredentialBlob: secret.as_mut_ptr(),
        Persist: CRED_PERSIST_SESSION,
        UserName: if username.is_empty() {
            PWSTR::null()
        } else {
            PWSTR::from_raw(username.as_mut_ptr())
        },
        ..CREDENTIALW::default()
    };
    // SAFETY: All fields reference terminated strings or owned buffers alive through this call.
    assert!(
        unsafe { CredWriteW(&credential, 0) }.is_ok(),
        "test credential write must succeed"
    );
    let mut credentials = ptr::null_mut();
    // SAFETY: The target and out-pointer are valid throughout the call.
    let read = unsafe {
        CredReadW(
            PCWSTR::from_raw(target.0.as_ptr()),
            CRED_TYPE_GENERIC,
            None,
            &mut credentials,
        )
    };
    assert!(read.is_ok(), "test credential read must succeed");
    assert!(
        !credentials.is_null(),
        "a written credential must be returned"
    );
    // SAFETY: CredReadW returned owned native storage valid until CredFree below.
    let copied = unsafe { username_and_password(&*credentials) };
    // SAFETY: This is the allocation returned by the successful CredReadW call.
    unsafe { CredFree(credentials.cast()) };
    copied.unwrap_or_else(|_| panic!("native credential copying must succeed"))
}

#[test]
fn credential_copy_accepts_null_optional_fields() {
    for size in [0, 17] {
        let credential = CREDENTIALW {
            CredentialBlobSize: size,
            ..CREDENTIALW::default()
        };
        // SAFETY: Null optional fields require no backing storage.
        let copied = unsafe { username_and_password(&credential) }
            .unwrap_or_else(|_| panic!("null fields must decode successfully"));
        assert!(copied.0.is_empty(), "a null account must be empty");
        assert!(copied.1.is_empty(), "a null blob must be empty");
    }
}

#[test]
fn credential_copy_owns_unicode_and_binary_buffers() {
    let mut username = "owned 账户🦀"
        .encode_utf16()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let mut blob = vec![0, 0xff, 0x61, 0];
    let credential = CREDENTIALW {
        UserName: PWSTR::from_raw(username.as_mut_ptr()),
        CredentialBlob: blob.as_mut_ptr(),
        CredentialBlobSize: blob.len() as u32,
        ..CREDENTIALW::default()
    };
    // SAFETY: Both buffers remain valid and the username is terminated.
    let copied = unsafe { username_and_password(&credential) }
        .unwrap_or_else(|_| panic!("valid owned fields must copy"));
    username.fill(0);
    blob.fill(0);
    assert!(
        copied.0 == "owned 账户🦀",
        "the account copy must be independent"
    );
    assert!(
        copied.1 == [0, 0xff, 0x61, 0],
        "the blob copy must be independent"
    );
}

#[test]
fn credential_copy_rejects_invalid_utf16_without_exposing_content() {
    let mut username = [0xd800, 0];
    let credential = CREDENTIALW {
        UserName: PWSTR::from_raw(username.as_mut_ptr()),
        ..CREDENTIALW::default()
    };
    // SAFETY: The buffer is readable and terminated, though it contains invalid UTF-16.
    assert!(unsafe { username_and_password(&credential) }.is_err());
}
