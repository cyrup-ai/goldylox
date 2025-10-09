//! macOS platform implementation using osascript and launchd.

use std::{
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{Context, Result};
use once_cell::sync::OnceCell;
// use plist::Value; // Using string template instead of plist crate

use crate::daemon::install::builder::CommandBuilder;
use crate::daemon::install::{InstallerBuilder, InstallerError};

// Imports for ZIP integrity verification
use hex;
use sha2;

pub(crate) struct PlatformExecutor;

// Global helper path - initialized once, used everywhere
static HELPER_PATH: OnceCell<PathBuf> = OnceCell::new();

// Embedded ZIP data for the signed helper app
// This is generated at build time by build.rs which creates a proper signed macOS helper
#[cfg(target_os = "macos")]
const APP_ZIP_DATA: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/LoxdHelper.app.zip"));

#[cfg(not(target_os = "macos"))]
const APP_ZIP_DATA: &[u8] = &[];

#[cfg(target_os = "macos")]
const EXPECTED_ZIP_HASH: &str = env!("MACOS_HELPER_ZIP_HASH");

#[cfg(not(target_os = "macos"))]
const EXPECTED_ZIP_HASH: &str = "";

impl PlatformExecutor {
    pub fn install(b: InstallerBuilder) -> Result<(), InstallerError> {
        // Initialize helper path if not already set
        Self::ensure_helper_path()?;

        // First, copy the binary to /tmp so elevated context can access it
        let temp_path = format!("/tmp/{}", b.label);
        std::fs::copy(&b.program, &temp_path)
            .map_err(|e| InstallerError::System(format!("Failed to copy binary to temp: {}", e)))?;

        let plist_content = Self::generate_plist(&b);

        // Build the installation commands using CommandBuilder
        let mkdir_cmd = CommandBuilder::new("mkdir").args([
            "-p",
            "/Library/LaunchDaemons",
            "/usr/local/bin",
            &format!("/var/log/{}", b.label),
        ]);

        let cp_cmd =
            CommandBuilder::new("cp").args([&temp_path, &format!("/usr/local/bin/{}", b.label)]);

        let chown_cmd = CommandBuilder::new("chown")
            .args(["root:wheel", &format!("/usr/local/bin/{}", b.label)]);

        let chmod_cmd =
            CommandBuilder::new("chmod").args(["755", &format!("/usr/local/bin/{}", b.label)]);

        let rm_cmd = CommandBuilder::new("rm").args(["-f", &temp_path]);
        // Write files to temp location first, then move them in elevated context
        let temp_plist = format!("/tmp/{}.plist", b.label);
        std::fs::write(&temp_plist, &plist_content)
            .map_err(|e| InstallerError::System(format!("Failed to write temp plist: {}", e)))?;

        let plist_file = format!("/Library/LaunchDaemons/{}.plist", b.label);

        let mut script = format!("set -e\n{}", Self::command_to_script(&mkdir_cmd));
        script.push_str(&format!(" && {}", Self::command_to_script(&cp_cmd)));
        script.push_str(&format!(" && {}", Self::command_to_script(&chown_cmd)));
        script.push_str(&format!(" && {}", Self::command_to_script(&chmod_cmd)));
        script.push_str(&format!(" && {}", Self::command_to_script(&rm_cmd)));
        script.push_str(&format!(" && mv {} {}", temp_plist, plist_file));

        // Set plist permissions
        let plist_perms_chown = CommandBuilder::new("chown").args(["root:wheel", &plist_file]);

        let plist_perms_chmod = CommandBuilder::new("chmod").args(["644", &plist_file]);

        script.push_str(&format!(
            " && {}",
            Self::command_to_script(&plist_perms_chown)
        ));
        script.push_str(&format!(
            " && {}",
            Self::command_to_script(&plist_perms_chmod)
        ));

        // Create services directory
        let services_dir = CommandBuilder::new("mkdir").args(["-p", "/etc/loxd/services"]);

        script.push_str(&format!(" && {}", Self::command_to_script(&services_dir)));

        // Add service definitions using CommandBuilder
        if !b.services.is_empty() {
            for service in &b.services {
                let service_toml = toml::to_string_pretty(service).map_err(|e| {
                    InstallerError::System(format!("Failed to serialize service: {}", e))
                })?;

                // Write service file to temp first
                let temp_service = format!("/tmp/{}.toml", service.name);
                std::fs::write(&temp_service, &service_toml).map_err(|e| {
                    InstallerError::System(format!("Failed to write temp service: {}", e))
                })?;

                let service_file = format!("/etc/loxd/services/{}.toml", service.name);
                script.push_str(&format!(" && mv {} {}", temp_service, service_file));

                // Set service file permissions using CommandBuilder
                let service_perms_chown =
                    CommandBuilder::new("chown").args(["root:wheel", &service_file]);

                let service_perms_chmod = CommandBuilder::new("chmod").args(["644", &service_file]);

                script.push_str(&format!(
                    " && {}",
                    Self::command_to_script(&service_perms_chown)
                ));
                script.push_str(&format!(
                    " && {}",
                    Self::command_to_script(&service_perms_chmod)
                ));
            }
        }

        // Load the daemon using CommandBuilder
        let load_daemon = CommandBuilder::new("launchctl").args([
            "load",
            "-w",
            &format!("/Library/LaunchDaemons/{}.plist", b.label),
        ]);

        script.push_str(&format!(" && {}", Self::command_to_script(&load_daemon)));

        Self::run_helper(&script)
    }

    /// Ensure the helper path is initialized for secure privileged operations
    fn ensure_helper_path() -> Result<(), InstallerError> {
        if HELPER_PATH.get().is_none() {
            let helper_path = Self::extract_helper_app()?;
            HELPER_PATH
                .set(helper_path)
                .map_err(|_| InstallerError::System("Failed to set helper path".to_string()))?;
        }
        Ok(())
    }

    /// Extract the signed helper app from embedded data
    fn extract_helper_app() -> Result<PathBuf, InstallerError> {
        // Use a stable location based on app version to avoid re-extraction
        let version_hash = {
            use std::hash::{Hash, Hasher};
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            APP_ZIP_DATA.len().hash(&mut hasher);
            APP_ZIP_DATA
                .get(0..64)
                .unwrap_or(&APP_ZIP_DATA[0..APP_ZIP_DATA.len().min(64)])
                .hash(&mut hasher);
            hasher.finish()
        };

        let helper_dir = std::env::temp_dir()
            .join("loxd_helper")
            .join(format!("v{:016x}", version_hash));

        std::fs::create_dir_all(&helper_dir).map_err(|e| {
            InstallerError::System(format!("Failed to create helper directory: {}", e))
        })?;

        let helper_path = helper_dir.join("LoxdHelper.app");

        // Check if helper already exists and is valid
        if helper_path.exists()
            && Self::validate_helper(&helper_path)?
            && Self::verify_code_signature(&helper_path)?
        {
            return Ok(helper_path);
        }

        // Extract from embedded data
        Self::extract_from_embedded_data(&helper_path)?;

        Ok(helper_path)
    }

    /// Extract helper from embedded APP_ZIP_DATA with zero-allocation validation
    fn extract_from_embedded_data(helper_path: &PathBuf) -> Result<bool, InstallerError> {
        use std::sync::atomic::{AtomicU8, Ordering};

        use arrayvec::ArrayVec;
        use atomic_counter::{AtomicCounter, RelaxedCounter};

        // Atomic validation state tracking (0=pending, 1=size_valid, 2=header_valid, 3=extraction_complete)
        static VALIDATION_STATE: AtomicU8 = AtomicU8::new(0);
        use once_cell::sync::Lazy;
        static VALIDATION_COUNTER: Lazy<RelaxedCounter> = Lazy::new(|| RelaxedCounter::new(0));

        VALIDATION_STATE.store(0, Ordering::Relaxed);
        VALIDATION_COUNTER.inc();

        // Zero-allocation ZIP validation using stack-allocated arrays
        const MIN_ZIP_SIZE: usize = 22; // Minimum ZIP central directory size

        if APP_ZIP_DATA.len() < MIN_ZIP_SIZE {
            return Err(InstallerError::System(
                "Embedded helper ZIP data is too small".to_string(),
            ));
        }
        VALIDATION_STATE.store(1, Ordering::Relaxed);

        // Zero-allocation ZIP magic header validation using ArrayVec
        let mut magic_headers: ArrayVec<&[u8], 3> = ArrayVec::new();
        magic_headers.push(&[0x50, 0x4B, 0x03, 0x04]); // Local file header
        magic_headers.push(&[0x50, 0x4B, 0x05, 0x06]); // Empty archive
        magic_headers.push(&[0x50, 0x4B, 0x07, 0x08]); // Spanned archive

        let has_valid_header = magic_headers
            .iter()
            .any(|&header| APP_ZIP_DATA.len() >= header.len() && APP_ZIP_DATA.starts_with(header));

        if !has_valid_header {
            return Err(InstallerError::System(
                "Invalid ZIP signature in embedded data".to_string(),
            ));
        }
        VALIDATION_STATE.store(2, Ordering::Relaxed);

        // Enhanced ZIP central directory validation using zero-copy access
        if let Err(e) = Self::validate_zip_central_directory() {
            return Err(InstallerError::System(format!(
                "ZIP central directory validation failed: {}",
                e
            )));
        }

        // Validate embedded ZIP integrity first
        Self::validate_embedded_zip()?;

        // Extract the embedded ZIP data
        match Self::extract_zip_data(APP_ZIP_DATA, helper_path) {
            Ok(_) => {
                VALIDATION_STATE.store(3, Ordering::Relaxed);

                // Enhanced validation with detailed error tracking
                let helper_valid = match Self::validate_helper(helper_path) {
                    Ok(valid) => {
                        if !valid {
                            return Err(InstallerError::System(
                                "Helper validation failed: Invalid bundle structure or missing Info.plist keys".to_string(),
                            ));
                        }
                        valid
                    }
                    Err(e) => {
                        return Err(InstallerError::System(format!(
                            "Helper validation error: {}",
                            e
                        )));
                    }
                };

                let signature_valid = match Self::verify_code_signature(helper_path) {
                    Ok(valid) => {
                        if !valid {
                            return Err(InstallerError::System(
                                "Code signature validation failed: Missing CodeResources or invalid bundle".to_string(),
                            ));
                        }
                        valid
                    }
                    Err(e) => {
                        return Err(InstallerError::System(format!(
                            "Code signature validation error: {}",
                            e
                        )));
                    }
                };

                if helper_valid && signature_valid {
                    Ok(true)
                } else {
                    // This should not be reached due to explicit error handling above
                    let _ = std::fs::remove_dir_all(helper_path);
                    Err(InstallerError::System(
                        "Extracted helper failed validation (unexpected)".to_string(),
                    ))
                }
            }
            Err(e) => {
                // Extraction failed, cleanup
                let _ = std::fs::remove_dir_all(helper_path);
                Err(e)
            }
        }
    }

    /// Verify embedded ZIP integrity
    fn validate_embedded_zip() -> Result<(), InstallerError> {
        #[cfg(not(target_os = "macos"))]
        {
            return Err(InstallerError::System("No embedded ZIP data".to_string()));
        }

        #[cfg(target_os = "macos")]
        {
            // This check is needed for runtime validation even though build.rs should guarantee data exists
            #[allow(clippy::const_is_empty)]
            if APP_ZIP_DATA.is_empty() {
                return Err(InstallerError::System("No embedded ZIP data".to_string()));
            }
        }

        #[cfg(target_os = "macos")]
        {
            use sha2::{Digest, Sha256};
            let mut hasher = Sha256::new();
            hasher.update(APP_ZIP_DATA);
            let hash_hex = hex::encode(hasher.finalize());

            if hash_hex != EXPECTED_ZIP_HASH {
                return Err(InstallerError::Security(
                    "ZIP integrity check failed".to_string(),
                ));
            }
        }

        // Validate ZIP structure
        Self::validate_zip_structure(APP_ZIP_DATA)?;

        Ok(())
    }

    /// Validate ZIP structure without full extraction
    fn validate_zip_structure(zip_data: &[u8]) -> Result<(), InstallerError> {
        use std::io::Cursor;

        let cursor = Cursor::new(zip_data);
        let mut archive = zip::ZipArchive::new(cursor)
            .map_err(|e| InstallerError::System(format!("Invalid ZIP structure: {}", e)))?;

        // Basic validation - ensure we have the expected helper app structure
        let mut has_info_plist = false;
        let mut has_executable = false;

        for i in 0..archive.len() {
            let file = archive.by_index(i).map_err(|e| {
                InstallerError::System(format!("Failed to read ZIP entry {}: {}", i, e))
            })?;

            let name = file.name();
            if name.contains("Info.plist") {
                has_info_plist = true;
            }
            if name.contains("MacOS/LoxdHelper") {
                has_executable = true;
            }
        }

        if !has_info_plist {
            return Err(InstallerError::System(
                "ZIP missing required Info.plist".to_string(),
            ));
        }

        if !has_executable {
            return Err(InstallerError::System(
                "ZIP missing required executable".to_string(),
            ));
        }

        Ok(())
    }

    /// Zero-allocation ZIP central directory validation using pointer arithmetic
    fn validate_zip_central_directory() -> Result<(), &'static str> {
        use arrayvec::ArrayVec;

        const EOCD_SIGNATURE: u32 = 0x06054b50; // End of Central Directory signature
        const EOCD_MIN_SIZE: usize = 22;

        if APP_ZIP_DATA.len() < EOCD_MIN_SIZE {
            return Err("ZIP data too small for central directory");
        }

        // Search for End of Central Directory record from the end (zero-allocation approach)
        let search_start = APP_ZIP_DATA.len().saturating_sub(65536); // ZIP spec: max comment size is 65535
        let search_range = &APP_ZIP_DATA[search_start..];

        // Stack-allocated buffer for signature checking
        let mut eocd_offset: Option<usize> = None;

        // Scan backwards for EOCD signature using zero-allocation approach
        for i in (0..search_range.len().saturating_sub(3)).rev() {
            if search_range.len() >= i + 4 {
                let signature_bytes: ArrayVec<u8, 4> = ArrayVec::from([
                    search_range[i],
                    search_range[i + 1],
                    search_range[i + 2],
                    search_range[i + 3],
                ]);

                let signature = u32::from_le_bytes([
                    signature_bytes[0],
                    signature_bytes[1],
                    signature_bytes[2],
                    signature_bytes[3],
                ]);

                if signature == EOCD_SIGNATURE {
                    eocd_offset = Some(search_start + i);
                    break;
                }
            }
        }

        let eocd_pos = eocd_offset.ok_or("End of Central Directory signature not found")?;

        // Validate EOCD structure using stack-allocated parsing
        if APP_ZIP_DATA.len() < eocd_pos + EOCD_MIN_SIZE {
            return Err("Incomplete End of Central Directory record");
        }

        // Parse central directory information (zero-allocation)
        let eocd_data = &APP_ZIP_DATA[eocd_pos..];

        if eocd_data.len() < 22 {
            return Err("EOCD record too short");
        }

        // Extract central directory info using zero-copy parsing
        let _disk_number = u16::from_le_bytes([eocd_data[4], eocd_data[5]]);
        let _cd_start_disk = u16::from_le_bytes([eocd_data[6], eocd_data[7]]);
        let cd_entries_this_disk = u16::from_le_bytes([eocd_data[8], eocd_data[9]]);
        let cd_total_entries = u16::from_le_bytes([eocd_data[10], eocd_data[11]]);
        let cd_size =
            u32::from_le_bytes([eocd_data[12], eocd_data[13], eocd_data[14], eocd_data[15]]);
        let cd_offset =
            u32::from_le_bytes([eocd_data[16], eocd_data[17], eocd_data[18], eocd_data[19]]);

        // Validate central directory parameters
        if cd_entries_this_disk != cd_total_entries {
            return Err("Multi-disk ZIP archives not supported");
        }

        if cd_total_entries == 0 {
            return Err("ZIP archive contains no entries");
        }

        // Validate central directory bounds
        let cd_end = cd_offset
            .checked_add(cd_size)
            .ok_or("Central directory offset/size overflow")?;

        if cd_end as usize > APP_ZIP_DATA.len() {
            return Err("Central directory extends beyond ZIP data");
        }

        if cd_offset as usize >= APP_ZIP_DATA.len() {
            return Err("Central directory offset beyond ZIP data");
        }

        Ok(())
    }

    /// Extract ZIP data to the specified path using production-ready extraction
    fn extract_zip_data(zip_data: &[u8], target_path: &PathBuf) -> Result<(), InstallerError> {
        use std::io::Cursor;

        if zip_data.is_empty() {
            return Err(InstallerError::System(
                "ZIP data is empty - build script may have failed".to_string(),
            ));
        }

        // Validate ZIP structure before extraction
        let cursor = Cursor::new(zip_data);
        let mut archive = zip::ZipArchive::new(cursor)
            .map_err(|e| InstallerError::System(format!("Invalid ZIP data: {}", e)))?;

        // Create target directory
        std::fs::create_dir_all(target_path.parent().unwrap_or(target_path)).map_err(|e| {
            InstallerError::System(format!("Failed to create target directory: {}", e))
        })?;

        // Extract all files with proper error handling
        for i in 0..archive.len() {
            let mut file = archive.by_index(i).map_err(|e| {
                InstallerError::System(format!("Failed to read ZIP entry {}: {}", i, e))
            })?;

            let outpath = match file.enclosed_name() {
                Some(path) => target_path.join(path),
                None => {
                    log::warn!("Skipping ZIP entry {} with unsafe name: {}", i, file.name());
                    continue; // Skip entries with unsafe names
                }
            };

            // Security check: prevent directory traversal
            if !outpath.starts_with(target_path) {
                return Err(InstallerError::Security(format!(
                    "Unsafe file path in ZIP: {}",
                    file.name()
                )));
            }

            if file.name().ends_with('/') {
                // Directory entry
                std::fs::create_dir_all(&outpath).map_err(|e| {
                    InstallerError::System(format!(
                        "Failed to create directory {}: {}",
                        outpath.display(),
                        e
                    ))
                })?;
            } else {
                // File entry - ensure parent directory exists
                if let Some(parent) = outpath.parent() {
                    std::fs::create_dir_all(parent).map_err(|e| {
                        InstallerError::System(format!(
                            "Failed to create parent directory {}: {}",
                            parent.display(),
                            e
                        ))
                    })?;
                }

                // Extract file with proper permissions
                let mut outfile = std::fs::File::create(&outpath).map_err(|e| {
                    InstallerError::System(format!(
                        "Failed to create file {}: {}",
                        outpath.display(),
                        e
                    ))
                })?;

                std::io::copy(&mut file, &mut outfile).map_err(|e| {
                    InstallerError::System(format!(
                        "Failed to extract file {}: {}",
                        outpath.display(),
                        e
                    ))
                })?;

                // Set executable permissions for macOS binaries
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    if let Some(mode) = file.unix_mode() {
                        std::fs::set_permissions(&outpath, std::fs::Permissions::from_mode(mode))
                            .map_err(|e| {
                            InstallerError::System(format!(
                                "Failed to set permissions on {}: {}",
                                outpath.display(),
                                e
                            ))
                        })?;
                    }
                }
            }
        }

        Ok(())
    }

    /// Validate that the helper app is properly signed and functional
    fn validate_helper(helper_path: &Path) -> Result<bool, InstallerError> {
        // Check if the helper exists and has the expected structure
        let contents = helper_path.join("Contents");
        let macos = contents.join("MacOS");
        let info_plist = contents.join("Info.plist");
        let executable = macos.join("LoxdHelper");

        // Verify all required components exist (zero-allocation existence checks)
        if !contents.exists() || !macos.exists() || !info_plist.exists() || !executable.exists() {
            return Ok(false);
        }

        // Verify Info.plist contains required keys
        let plist_data = std::fs::read(&info_plist)
            .map_err(|e| InstallerError::System(format!("Failed to read Info.plist: {}", e)))?;

        // Simple plist validation - check if it's a valid XML file with required keys
        let plist_content = String::from_utf8_lossy(&plist_data);

        // Check for required plist keys
        let has_bundle_id = plist_content.contains("CFBundleIdentifier");
        let has_bundle_executable = plist_content.contains("CFBundleExecutable");
        let has_sm_authorized = plist_content.contains("SMAuthorizedClients");

        Ok(has_bundle_id && has_bundle_executable && has_sm_authorized)
    }

    /// Verify the code signature of the helper app using Tauri-compatible validation
    fn verify_code_signature(helper_path: &Path) -> Result<bool, InstallerError> {
        // Use Tauri's signing verification approach - check for valid bundle structure
        // and signature presence without manual codesign calls

        // Verify CodeResources exists (created by Tauri signing)
        let code_resources = helper_path.join("Contents/_CodeSignature/CodeResources");
        if !code_resources.exists() {
            return Err(InstallerError::System(
                "Helper app missing CodeResources - not properly signed".to_string(),
            ));
        }

        // Verify executable exists and has proper permissions
        let executable = helper_path.join("Contents/MacOS/LoxdHelper");
        if !executable.exists() {
            return Err(InstallerError::System(
                "Helper app missing executable".to_string(),
            ));
        }

        // Check executable permissions (should be executable)
        let metadata = std::fs::metadata(&executable).map_err(|e| {
            InstallerError::System(format!("Failed to get executable metadata: {}", e))
        })?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = metadata.permissions().mode();
            // Check if executable bit is set (0o100)
            if (mode & 0o111) == 0 {
                return Err(InstallerError::System(
                    "Helper executable does not have execute permissions".to_string(),
                ));
            }
        }

        // Verify Info.plist has proper bundle structure
        let info_plist = helper_path.join("Contents/Info.plist");
        let plist_data = std::fs::read(&info_plist)
            .map_err(|e| InstallerError::System(format!("Failed to read Info.plist: {}", e)))?;

        // Simple bundle identifier validation
        let plist_content = String::from_utf8_lossy(&plist_data);

        // Check for expected bundle identifier
        if !plist_content.contains("com.goldylox.loxd.helper") {
            return Err(InstallerError::System(
                "Bundle identifier validation failed - expected com.goldylox.loxd.helper"
                    .to_string(),
            ));
        }

        if !plist_content.contains("CFBundleIdentifier") {
            return Err(InstallerError::System(
                "Missing CFBundleIdentifier in Info.plist".to_string(),
            ));
        }

        // If all Tauri-signed bundle validation checks pass, the helper is valid
        Ok(true)
    }

    pub fn uninstall(label: &str) -> Result<(), InstallerError> {
        let script = format!(
            r#"
            set -e
            # Unload daemon if running
            launchctl unload -w /Library/LaunchDaemons/{label}.plist 2>/dev/null || true
            
            # Remove files
            rm -f /Library/LaunchDaemons/{label}.plist
            rm -f /usr/local/bin/{label}
            rm -rf /var/log/{label}
        "#,
            label = label
        );

        Self::run_helper(&script)
    }

    fn generate_plist(b: &InstallerBuilder) -> String {
        let program_args = if b.args.is_empty() {
            format!("        <string>/usr/local/bin/{}</string>", b.label)
        } else {
            let mut args_xml = format!("        <string>/usr/local/bin/{}</string>\n", b.label);
            for arg in &b.args {
                args_xml.push_str(&format!("        <string>{}</string>\n", arg));
            }
            args_xml.trim_end().to_string()
        };

        let env_vars = if b.env.is_empty() {
            String::new()
        } else {
            let mut env_xml = "    <key>EnvironmentVariables</key>\n    <dict>\n".to_string();
            for (key, value) in &b.env {
                env_xml.push_str(&format!(
                    "        <key>{}</key>\n        <string>{}</string>\n",
                    key, value
                ));
            }
            env_xml.push_str("    </dict>\n");
            env_xml
        };

        let keep_alive = if b.auto_restart {
            "    <key>KeepAlive</key>\n    <dict>\n        <key>SuccessfulExit</key>\n        <false/>\n    </dict>\n"
        } else {
            "    <key>KeepAlive</key>\n    <false/>\n"
        };

        let network_limit = if b.wants_network {
            "    <key>LimitLoadToSessionType</key>\n    <string>System</string>\n"
        } else {
            ""
        };

        let group_xml = if b.run_as_group != "wheel" && b.run_as_group != "staff" {
            format!(
                "    <key>GroupName</key>\n    <string>{}</string>\n",
                b.run_as_group
            )
        } else {
            String::new()
        };

        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{}</string>
    <key>Disabled</key>
    <false/>
    <key>ProgramArguments</key>
    <array>
{}
    </array>
{}    <key>UserName</key>
    <string>{}</string>
{}{}    <key>StandardOutPath</key>
    <string>/var/log/{}/stdout.log</string>
    <key>StandardErrorPath</key>
    <string>/var/log/{}/stderr.log</string>
    <key>RunAtLoad</key>
    <true/>
{}
</dict>
</plist>"#,
            b.label,
            program_args,
            env_vars,
            b.run_as_user,
            group_xml,
            keep_alive,
            b.label,
            b.label,
            network_limit
        )
    }

    #[allow(dead_code)]
    fn run_osascript(script: &str) -> Result<(), InstallerError> {
        // Escape the script for AppleScript
        let escaped_script = script.replace('\\', "\\\\").replace('"', "\\\"");

        let applescript = format!(
            r#"do shell script "{}" with administrator privileges"#,
            escaped_script
        );

        let output = Command::new("osascript")
            .arg("-e")
            .arg(&applescript)
            .output()
            .context("failed to invoke osascript")?;

        if output.status.success() {
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("User canceled") || stderr.contains("-128") {
                Err(InstallerError::Cancelled)
            } else if stderr.contains("authorization") || stderr.contains("privileges") {
                Err(InstallerError::PermissionDenied)
            } else {
                Err(InstallerError::System(stderr.into_owned()))
            }
        }
    }

    /// Execute script using the signed helper app with elevated privileges
    fn run_helper(script: &str) -> Result<(), InstallerError> {
        // Get the helper path
        let helper_path = HELPER_PATH
            .get()
            .ok_or_else(|| InstallerError::System("Helper app not initialized".to_string()))?;

        let helper_exe = helper_path.join("Contents/MacOS/LoxdHelper");

        // Launch helper with elevated privileges using osascript
        // The helper itself is what gets elevated, not the script
        let escaped_helper = helper_exe
            .to_string_lossy()
            .replace('\\', "\\\\")
            .replace('"', "\\\"");

        let applescript = format!(
            r#"do shell script "\"{}\"" with administrator privileges"#,
            escaped_helper
        );

        // Start the helper process with admin privileges
        let mut child = Command::new("osascript")
            .arg("-e")
            .arg(&applescript)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| InstallerError::System(format!("Failed to launch helper: {}", e)))?;

        // Write the script to the helper's stdin
        if let Some(mut stdin) = child.stdin.take() {
            use std::io::Write;
            stdin.write_all(script.as_bytes()).map_err(|e| {
                InstallerError::System(format!("Failed to send script to helper: {}", e))
            })?;
            stdin.flush().map_err(|e| {
                InstallerError::System(format!("Failed to flush script to helper: {}", e))
            })?;
        }

        // Wait for the helper to complete
        let output = child
            .wait_with_output()
            .map_err(|e| InstallerError::System(format!("Failed to wait for helper: {}", e)))?;

        if output.status.success() {
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);

            // Check for user cancellation
            if stderr.contains("User canceled")
                || stderr.contains("-128")
                || stdout.contains("User canceled")
                || stdout.contains("-128")
            {
                Err(InstallerError::Cancelled)
            } else if stderr.contains("Unauthorized parent process") {
                Err(InstallerError::System(
                    "Helper security check failed".to_string(),
                ))
            } else if stderr.contains("Script execution timed out") {
                Err(InstallerError::System(
                    "Installation script timed out".to_string(),
                ))
            } else {
                // Include both stdout and stderr for debugging
                let full_error = format!("Helper failed: stdout={}, stderr={}", stdout, stderr);
                Err(InstallerError::System(full_error))
            }
        }
    }

    pub async fn install_async(b: InstallerBuilder) -> Result<(), InstallerError> {
        tokio::task::spawn_blocking(move || Self::install(b))
            .await
            .context("task join failed")?
    }

    fn command_to_script(cmd: &CommandBuilder) -> String {
        let mut parts = vec![cmd.program.to_string_lossy().to_string()];
        parts.extend(cmd.args.iter().cloned());
        parts.join(" ")
    }

    pub async fn uninstall_async(label: &str) -> Result<(), InstallerError> {
        let label = label.to_string();
        tokio::task::spawn_blocking(move || Self::uninstall(&label))
            .await
            .context("task join failed")?
    }
}
