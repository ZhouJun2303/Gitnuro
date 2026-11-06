//! API keys, proxy passwords, and forge tokens.
//!
//! The settings file keeps those fields empty. The values live in Windows
//! Credential Manager, the macOS keychain, or a directory named by
//! `AWEGIT_SECRET_DIR` (tests and the browser lab). An empty value deletes
//! the stored secret so a cleared token stays cleared.

use std::path::{Path, PathBuf};

use crate::settings::Settings;

const FIXED: [&str; 6] = [
    "ai_api_key",
    "proxy_password",
    "github_token",
    "gitlab_token",
    "bitbucket_token",
    "azure_token",
];

/// Write any non-empty secret already present in `settings` into the vault.
/// Memory is left unchanged so the caller can still use the value.
pub fn import_plaintext(settings: &Settings) -> Result<bool, String> {
    let mut changed = false;
    for index in 0..slot_count(settings) {
        let value = slot_get(settings, index);
        if value.is_empty() {
            continue;
        }
        let key = slot_key(settings, index);
        write_secret(&key, value)?;
        changed = true;
    }
    Ok(changed)
}

/// Fill empty secret fields from the vault. Fields that already have a value stay.
pub fn fill(settings: &mut Settings) -> Result<(), String> {
    for index in 0..slot_count(settings) {
        if !slot_get(settings, index).is_empty() {
            continue;
        }
        let key = slot_key(settings, index);
        if let Some(value) = read_secret(&key)? {
            *slot_mut(settings, index) = value;
        }
    }
    Ok(())
}

/// Store every secret, delete keys whose value is now empty, then clear the
/// fields on this struct. Call it on the clone that is about to be written.
pub fn store_and_clear(settings: &mut Settings) -> Result<(), String> {
    for index in 0..slot_count(settings) {
        let key = slot_key(settings, index);
        let value = slot_get(settings, index).to_string();
        if value.is_empty() {
            delete_secret(&key)?;
        } else {
            write_secret(&key, &value)?;
        }
        slot_mut(settings, index).clear();
    }
    Ok(())
}

fn slot_count(settings: &Settings) -> usize {
    FIXED.len() + settings.accounts.len()
}

fn slot_key(settings: &Settings, index: usize) -> String {
    if index < FIXED.len() {
        return FIXED[index].to_string();
    }
    let id = settings.accounts[index - FIXED.len()].id.trim();
    if id.is_empty() {
        format!("account:#{index}")
    } else {
        format!("account:{id}")
    }
}

fn slot_get(settings: &Settings, index: usize) -> &str {
    match index {
        0 => settings.ai_api_key.as_str(),
        1 => settings.proxy_password.as_str(),
        2 => settings.github_token.as_str(),
        3 => settings.gitlab_token.as_str(),
        4 => settings.bitbucket_token.as_str(),
        5 => settings.azure_token.as_str(),
        n => settings.accounts[n - FIXED.len()].token.as_str(),
    }
}

fn slot_mut(settings: &mut Settings, index: usize) -> &mut String {
    match index {
        0 => &mut settings.ai_api_key,
        1 => &mut settings.proxy_password,
        2 => &mut settings.github_token,
        3 => &mut settings.gitlab_token,
        4 => &mut settings.bitbucket_token,
        5 => &mut settings.azure_token,
        n => &mut settings.accounts[n - FIXED.len()].token,
    }
}

enum Vault {
    Dir(PathBuf),
    #[cfg(all(windows, target_pointer_width = "64"))]
    Windows,
    #[cfg(target_os = "macos")]
    Macos,
}

fn open_vault() -> Result<Vault, String> {
    if let Some(dir) = std::env::var_os("AWEGIT_SECRET_DIR") {
        let dir = PathBuf::from(dir);
        if dir.as_os_str().is_empty() {
            return Err("AWEGIT_SECRET_DIR is empty".into());
        }
        return Ok(Vault::Dir(dir));
    }
    if cfg!(test) {
        return Err("AWEGIT_SECRET_DIR is not set".into());
    }
    native_vault()
}

#[cfg(all(windows, target_pointer_width = "64"))]
fn native_vault() -> Result<Vault, String> {
    Ok(Vault::Windows)
}

#[cfg(target_os = "macos")]
fn native_vault() -> Result<Vault, String> {
    Ok(Vault::Macos)
}

#[cfg(not(any(all(windows, target_pointer_width = "64"), target_os = "macos")))]
fn native_vault() -> Result<Vault, String> {
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local").join("share")))
        .unwrap_or_else(|| PathBuf::from("."));
    Ok(Vault::Dir(base.join("awegit").join("secrets")))
}

fn write_secret(key: &str, value: &str) -> Result<(), String> {
    match open_vault()? {
        Vault::Dir(dir) => write_file(&dir, key, value),
        #[cfg(all(windows, target_pointer_width = "64"))]
        Vault::Windows => wincred::write(key, value),
        #[cfg(target_os = "macos")]
        Vault::Macos => macos::write(key, value),
    }
}

fn read_secret(key: &str) -> Result<Option<String>, String> {
    match open_vault()? {
        Vault::Dir(dir) => read_file(&dir, key),
        #[cfg(all(windows, target_pointer_width = "64"))]
        Vault::Windows => wincred::read(key),
        #[cfg(target_os = "macos")]
        Vault::Macos => macos::read(key),
    }
}

fn delete_secret(key: &str) -> Result<(), String> {
    match open_vault()? {
        Vault::Dir(dir) => delete_file(&dir, key),
        #[cfg(all(windows, target_pointer_width = "64"))]
        Vault::Windows => wincred::delete(key),
        #[cfg(target_os = "macos")]
        Vault::Macos => macos::delete(key),
    }
}

fn file_name(key: &str) -> String {
    let mut name = String::new();
    for ch in key.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
            name.push(ch);
        } else {
            name.push('_');
        }
    }
    if name.is_empty() {
        name.push_str("secret");
    }
    format!("{name}.secret")
}

fn write_file(dir: &Path, key: &str, value: &str) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|error| error.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700));
    }
    let name = file_name(key);
    let dest = dir.join(&name);
    let tmp = dir.join(format!("{name}.tmp"));
    {
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&tmp).map_err(|error| error.to_string())?;
        use std::io::Write;
        file.write_all(value.as_bytes()).map_err(|error| error.to_string())?;
        file.sync_all().map_err(|error| error.to_string())?;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600));
    }
    replace_file(&tmp, &dest)
}

fn read_file(dir: &Path, key: &str) -> Result<Option<String>, String> {
    let path = dir.join(file_name(key));
    match std::fs::read(&path) {
        Ok(bytes) => String::from_utf8(bytes)
            .map(Some)
            .map_err(|_| format!("stored secret {key} is not utf-8")),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.to_string()),
    }
}

fn delete_file(dir: &Path, key: &str) -> Result<(), String> {
    let path = dir.join(file_name(key));
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.to_string()),
    }
}

fn replace_file(from: &Path, to: &Path) -> Result<(), String> {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        let from_wide: Vec<u16> = from.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
        let to_wide: Vec<u16> = to.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
        let ok = unsafe { MoveFileExW(from_wide.as_ptr(), to_wide.as_ptr(), MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH) };
        if ok == 0 {
            return Err(std::io::Error::last_os_error().to_string());
        }
        return Ok(());
    }
    #[cfg(not(windows))]
    {
        let _ = std::fs::remove_file(to);
        std::fs::rename(from, to).map_err(|error| error.to_string())
    }
}

#[cfg(windows)]
const MOVEFILE_REPLACE_EXISTING: u32 = 0x1;
#[cfg(windows)]
const MOVEFILE_WRITE_THROUGH: u32 = 0x8;

#[cfg(windows)]
#[link(name = "kernel32")]
extern "system" {
    fn MoveFileExW(from: *const u16, to: *const u16, flags: u32) -> i32;
}

#[cfg(all(windows, target_pointer_width = "64"))]
mod wincred {
    const CRED_TYPE_GENERIC: u32 = 1;
    const CRED_PERSIST_LOCAL_MACHINE: u32 = 2;
    const ERROR_NOT_FOUND: i32 = 1168;

    #[repr(C)]
    struct CredentialW {
        flags: u32,
        type_: u32,
        target_name: *mut u16,
        comment: *mut u16,
        last_written: i64,
        credential_blob_size: u32,
        _pad: u32,
        credential_blob: *mut u8,
        persist: u32,
        attribute_count: u32,
        attributes: *mut std::ffi::c_void,
        target_alias: *mut u16,
        user_name: *mut u16,
    }

    const _: () = assert!(std::mem::size_of::<CredentialW>() == 80);

    #[link(name = "advapi32")]
    extern "system" {
        fn CredWriteW(credential: *const CredentialW, flags: u32) -> i32;
        fn CredReadW(target: *const u16, type_: u32, flags: u32, credential: *mut *mut CredentialW) -> i32;
        fn CredDeleteW(target: *const u16, type_: u32, flags: u32) -> i32;
        fn CredFree(buffer: *mut std::ffi::c_void);
    }

    pub fn write(key: &str, value: &str) -> Result<(), String> {
        let mut target = wide(&target_name(key));
        let mut blob = value.as_bytes().to_vec();
        let cred = CredentialW {
            flags: 0,
            type_: CRED_TYPE_GENERIC,
            target_name: target.as_mut_ptr(),
            comment: std::ptr::null_mut(),
            last_written: 0,
            credential_blob_size: blob.len() as u32,
            _pad: 0,
            credential_blob: blob.as_mut_ptr(),
            persist: CRED_PERSIST_LOCAL_MACHINE,
            attribute_count: 0,
            attributes: std::ptr::null_mut(),
            target_alias: std::ptr::null_mut(),
            user_name: std::ptr::null_mut(),
        };
        let ok = unsafe { CredWriteW(&cred, 0) };
        if ok == 0 {
            return Err(format!("credential write failed for {key}: {}", std::io::Error::last_os_error()));
        }
        Ok(())
    }

    pub fn read(key: &str) -> Result<Option<String>, String> {
        let target = wide(&target_name(key));
        let mut cred = std::ptr::null_mut();
        let ok = unsafe { CredReadW(target.as_ptr(), CRED_TYPE_GENERIC, 0, &mut cred) };
        if ok == 0 {
            let error = std::io::Error::last_os_error();
            if error.raw_os_error() == Some(ERROR_NOT_FOUND) {
                return Ok(None);
            }
            return Err(format!("credential read failed for {key}: {error}"));
        }
        if cred.is_null() {
            return Ok(None);
        }
        let text = unsafe {
            let size = (*cred).credential_blob_size as usize;
            let blob = (*cred).credential_blob;
            let copied = if blob.is_null() || size == 0 {
                Vec::new()
            } else {
                std::slice::from_raw_parts(blob, size).to_vec()
            };
            CredFree(cred as *mut std::ffi::c_void);
            copied
        };
        String::from_utf8(text)
            .map(Some)
            .map_err(|_| format!("stored secret {key} is not utf-8"))
    }

    pub fn delete(key: &str) -> Result<(), String> {
        let target = wide(&target_name(key));
        let ok = unsafe { CredDeleteW(target.as_ptr(), CRED_TYPE_GENERIC, 0) };
        if ok == 0 {
            let error = std::io::Error::last_os_error();
            if error.raw_os_error() == Some(ERROR_NOT_FOUND) {
                return Ok(());
            }
            return Err(format!("credential delete failed for {key}: {error}"));
        }
        Ok(())
    }

    fn target_name(key: &str) -> String {
        format!("com.zhoujun.awegit:{key}")
    }

    fn wide(text: &str) -> Vec<u16> {
        use std::os::windows::ffi::OsStrExt;
        std::ffi::OsStr::new(text).encode_wide().chain(std::iter::once(0)).collect()
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use std::os::raw::{c_char, c_void};

    const ERR_NOT_FOUND: i32 = -25300;
    const SERVICE: &[u8] = b"com.zhoujun.awegit";

    #[link(name = "Security", kind = "framework")]
    extern "C" {
        fn SecKeychainAddGenericPassword(
            keychain: *mut c_void,
            service_name_length: u32,
            service_name: *const c_char,
            account_name_length: u32,
            account_name: *const c_char,
            password_length: u32,
            password_data: *const c_void,
            item_ref: *mut *mut c_void,
        ) -> i32;
        fn SecKeychainFindGenericPassword(
            keychain: *mut c_void,
            service_name_length: u32,
            service_name: *const c_char,
            account_name_length: u32,
            account_name: *const c_char,
            password_length: *mut u32,
            password_data: *mut *mut c_void,
            item_ref: *mut *mut c_void,
        ) -> i32;
        fn SecKeychainItemDelete(item_ref: *mut c_void) -> i32;
        fn SecKeychainItemFreeContent(attr_list: *mut c_void, data: *mut c_void) -> i32;
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        fn CFRelease(item: *const c_void);
    }

    pub fn write(key: &str, value: &str) -> Result<(), String> {
        let _ = delete(key);
        let account = key.as_bytes();
        let secret = value.as_bytes();
        let status = unsafe {
            SecKeychainAddGenericPassword(
                std::ptr::null_mut(),
                SERVICE.len() as u32,
                SERVICE.as_ptr() as *const c_char,
                account.len() as u32,
                account.as_ptr() as *const c_char,
                secret.len() as u32,
                secret.as_ptr() as *const c_void,
                std::ptr::null_mut(),
            )
        };
        if status != 0 {
            return Err(format!("keychain write failed for {key} ({status})"));
        }
        Ok(())
    }

    pub fn read(key: &str) -> Result<Option<String>, String> {
        let account = key.as_bytes();
        let mut length = 0u32;
        let mut data = std::ptr::null_mut();
        let mut item = std::ptr::null_mut();
        let status = unsafe {
            SecKeychainFindGenericPassword(
                std::ptr::null_mut(),
                SERVICE.len() as u32,
                SERVICE.as_ptr() as *const c_char,
                account.len() as u32,
                account.as_ptr() as *const c_char,
                &mut length,
                &mut data,
                &mut item,
            )
        };
        if status == ERR_NOT_FOUND {
            return Ok(None);
        }
        if status != 0 {
            return Err(format!("keychain read failed for {key} ({status})"));
        }
        let copied = unsafe {
            let bytes = if data.is_null() || length == 0 {
                Vec::new()
            } else {
                std::slice::from_raw_parts(data as *const u8, length as usize).to_vec()
            };
            SecKeychainItemFreeContent(std::ptr::null_mut(), data);
            if !item.is_null() {
                CFRelease(item);
            }
            bytes
        };
        String::from_utf8(copied)
            .map(Some)
            .map_err(|_| format!("stored secret {key} is not utf-8"))
    }

    pub fn delete(key: &str) -> Result<(), String> {
        let account = key.as_bytes();
        let mut length = 0u32;
        let mut data = std::ptr::null_mut();
        let mut item = std::ptr::null_mut();
        let status = unsafe {
            SecKeychainFindGenericPassword(
                std::ptr::null_mut(),
                SERVICE.len() as u32,
                SERVICE.as_ptr() as *const c_char,
                account.len() as u32,
                account.as_ptr() as *const c_char,
                &mut length,
                &mut data,
                &mut item,
            )
        };
        if status == ERR_NOT_FOUND {
            return Ok(());
        }
        if status != 0 {
            return Err(format!("keychain delete failed for {key} ({status})"));
        }
        let deleted = unsafe {
            SecKeychainItemFreeContent(std::ptr::null_mut(), data);
            let deleted = SecKeychainItemDelete(item);
            if !item.is_null() {
                CFRelease(item);
            }
            deleted
        };
        if deleted != 0 && deleted != ERR_NOT_FOUND {
            return Err(format!("keychain delete failed for {key} ({deleted})"));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch() -> (PathBuf, Option<std::ffi::OsString>) {
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let dir = std::env::temp_dir().join(format!("awegit-secrets-{nanos}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let previous = std::env::var_os("AWEGIT_SECRET_DIR");
        unsafe { std::env::set_var("AWEGIT_SECRET_DIR", &dir) };
        (dir, previous)
    }

    fn restore(dir: &Path, previous: Option<std::ffi::OsString>) {
        match previous {
            Some(value) => unsafe { std::env::set_var("AWEGIT_SECRET_DIR", value) },
            None => unsafe { std::env::remove_var("AWEGIT_SECRET_DIR") },
        }
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn file_vault_roundtrips_and_an_empty_value_deletes() {
        let (dir, previous) = scratch();
        write_secret("github_token", "s3cret-值").unwrap();
        assert_eq!(read_secret("github_token").unwrap().as_deref(), Some("s3cret-值"));
        let stored = std::fs::read(dir.join("github_token.secret")).unwrap();
        assert_eq!(stored, b"s3cret-\xe5\x80\xbc");
        delete_secret("github_token").unwrap();
        assert_eq!(read_secret("github_token").unwrap(), None);
        delete_secret("github_token").unwrap();
        restore(&dir, previous);
    }

    #[test]
    fn tests_do_not_use_the_os_store_without_a_secret_dir() {
        let previous = std::env::var_os("AWEGIT_SECRET_DIR");
        unsafe { std::env::remove_var("AWEGIT_SECRET_DIR") };
        let error = write_secret("github_token", "should-not-land").unwrap_err();
        assert!(error.contains("AWEGIT_SECRET_DIR"), "{error}");
        assert!(!error.contains("should-not-land"), "{error}");
        match previous {
            Some(value) => unsafe { std::env::set_var("AWEGIT_SECRET_DIR", value) },
            None => {}
        }
    }
}
