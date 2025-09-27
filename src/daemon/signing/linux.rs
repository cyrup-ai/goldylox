//! Linux code signing implementation using GPG

#[cfg(feature = "daemon")]
use std::fs::File;
use std::io::{BufReader, Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};

use anyhow::{Context, Result, anyhow, bail};
use which::which;

use super::{PlatformConfig, SigningConfig};

/// Sign a binary on Linux using GPG
pub fn sign(config: &SigningConfig) -> Result<()> {
    let PlatformConfig::Linux { key_id, detached } = &config.platform else {
        bail!("Invalid platform config for Linux");
    };

    // Find gpg command
    let gpg = find_gpg()?;

    // Build GPG arguments
    let mut args = vec![];

    if *detached {
        args.push("--detach-sign".to_string());
        args.push("--armor".to_string());
    } else {
        args.push("--sign".to_string());
    }

    // Add key ID if specified
    if let Some(key) = key_id {
        args.push("--local-user".to_string());
        args.push(key.clone());
    }

    // Output file
    let sig_path = if *detached {
        config.binary_path.with_extension("sig")
    } else {
        config.output_path.with_extension("gpg")
    };

    args.push("--output".to_string());
    args.push(sig_path.to_string_lossy().to_string());

    // Input file
    args.push(config.binary_path.to_string_lossy().to_string());

    // Execute GPG
    let output = Command::new(&gpg)
        .args(&args)
        .output()
        .context("Failed to execute GPG")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("GPG signing failed: {}", stderr);
    }

    if *detached {
        println!(
            "Successfully created detached signature: {}",
            sig_path.display()
        );
    } else {
        println!("Successfully signed binary: {}", sig_path.display());
    }

    Ok(())
}

/// Verify a signed binary on Linux
pub fn verify(binary_path: &Path) -> Result<bool> {
    let gpg = match find_gpg() {
        Ok(cmd) => cmd,
        Err(_) => return Ok(false), // Can't verify without GPG
    };

    // Check for detached signature first
    let sig_path = binary_path.with_extension("sig");
    if sig_path.exists() {
        // Verify detached signature with proper error handling
        let sig_path_str = sig_path
            .to_str()
            .ok_or_else(|| anyhow!("Invalid UTF-8 in signature path: {:?}", sig_path))?;
        let binary_path_str = binary_path
            .to_str()
            .ok_or_else(|| anyhow!("Invalid UTF-8 in binary path: {:?}", binary_path))?;

        let output = Command::new(&gpg)
            .args(&["--verify", sig_path_str, binary_path_str])
            .output()
            .context("Failed to execute GPG verify")?;

        return Ok(output.status.success());
    }

    // Check for inline signed file
    let gpg_path = binary_path.with_extension("gpg");
    if gpg_path.exists() {
        let gpg_path_str = gpg_path
            .to_str()
            .ok_or_else(|| anyhow!("Invalid UTF-8 in GPG file path: {:?}", gpg_path))?;

        let output = Command::new(&gpg)
            .args(&["--verify", gpg_path_str])
            .output()
            .context("Failed to execute GPG verify")?;

        return Ok(output.status.success());
    }

    // No signature found
    Ok(false)
}

/// Find GPG command on the system
fn find_gpg() -> Result<String> {
    // Try gpg2 first, then gpg
    if let Ok(gpg2) = which("gpg2") {
        return Ok(gpg2.to_string_lossy().to_string());
    }

    if let Ok(gpg) = which("gpg") {
        return Ok(gpg.to_string_lossy().to_string());
    }

    bail!("GPG not found. Please install gpg or gpg2");
}

/// Sign AppImage with external detached GPG signature (memory-safe)
pub fn sign_appimage(appimage_path: &Path, key_id: Option<&str>) -> Result<()> {
    let gpg = find_gpg()?;

    // Validate AppImage exists and get file size safely
    if !appimage_path.exists() {
        bail!("AppImage not found: {}", appimage_path.display());
    }

    let metadata = appimage_path
        .metadata()
        .context("Failed to read AppImage metadata")?;

    let file_size = metadata.len();

    // Security: Limit file size to prevent DoS (100MB limit)
    const MAX_APPIMAGE_SIZE: u64 = 100 * 1024 * 1024; // 100MB
    if file_size > MAX_APPIMAGE_SIZE {
        bail!(
            "AppImage too large: {} bytes (max {} bytes)",
            file_size,
            MAX_APPIMAGE_SIZE
        );
    }

    // Memory-safe AppImage format validation using buffered reading
    let file = File::open(appimage_path).context("Failed to open AppImage file")?;
    let mut reader = BufReader::new(file);

    // Read only first 32 bytes for magic detection
    let mut header = [0u8; 32];
    reader
        .read_exact(&mut header)
        .context("Failed to read AppImage header")?;

    // Check AppImage magic bytes
    let is_appimage_v1 = header.starts_with(b"#!/bin/sh\n(exec ");
    let is_appimage_v2 = header.starts_with(&[0x41, 0x49, 0x02]); // AI\x02

    if !is_appimage_v1 && !is_appimage_v2 {
        bail!("Not a valid AppImage file - invalid magic bytes");
    }

    // Create detached signature for the AppImage
    let mut args = vec!["--detach-sign".to_string(), "--armor".to_string()];

    if let Some(key) = key_id {
        args.push("--local-user".to_string());
        args.push(key.to_string());
    }

    let sig_path = appimage_path.with_extension("AppImage.sig");
    args.push("--output".to_string());
    args.push(sig_path.to_string_lossy().to_string());
    args.push(appimage_path.to_string_lossy().to_string());

    let output = Command::new(&gpg)
        .args(&args)
        .output()
        .context("Failed to execute GPG for AppImage signing")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("AppImage GPG signing failed: {}", stderr);
    }

    println!(
        "Successfully created detached AppImage signature: {}",
        sig_path.display()
    );
    Ok(())
}

/// Generate a new GPG key for signing
pub fn generate_signing_key(name: &str, email: &str) -> Result<String> {
    let gpg = find_gpg()?;

    // Validate inputs to prevent GPG script injection
    if name.contains('\n') || name.contains('\r') || name.len() > 100 {
        bail!("Invalid name: contains newlines or too long");
    }
    if email.contains('\n') || email.contains('\r') || email.len() > 100 {
        bail!("Invalid email: contains newlines or too long");
    }

    // Create key generation script
    let key_script = format!(
        r#"
Key-Type: RSA
Key-Length: 4096
Subkey-Type: RSA
Subkey-Length: 4096
Name-Real: {}
Name-Email: {}
Expire-Date: 2y
%no-protection
%commit
"#,
        name, email
    );

    // Execute GPG with batch mode and proper process handling
    let mut child = Command::new(&gpg)
        .args(&["--batch", "--generate-key"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("Failed to spawn GPG process")?;

    // Write key script to stdin with proper error handling
    if let Some(stdin) = child.stdin.as_mut() {
        stdin
            .write_all(key_script.as_bytes())
            .context("Failed to write key generation script to GPG stdin")?;
    } else {
        bail!("Failed to get GPG stdin handle - process communication error");
    }

    let output = child
        .wait_with_output()
        .context("Failed to wait for GPG key generation")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("Failed to generate GPG key: {}", stderr);
    }

    // Get the key ID
    let list_output = Command::new(&gpg)
        .args(&["--list-secret-keys", "--keyid-format", "LONG", email])
        .output()
        .context("Failed to list GPG keys")?;

    let stdout = String::from_utf8_lossy(&list_output.stdout);

    // Parse key ID from output
    let key_id = stdout
        .lines()
        .find(|line| line.contains("sec"))
        .and_then(|line| line.split('/').nth(1))
        .and_then(|s| s.split_whitespace().next())
        .ok_or_else(|| anyhow!("Failed to parse generated key ID from GPG output"))?
        .to_string();

    println!("Generated GPG key: {}", key_id);
    Ok(key_id)
}

/// Export GPG public key for distribution
pub fn export_public_key(key_id: &str, output_path: &Path) -> Result<()> {
    let gpg = find_gpg()?;

    let output_path_str = output_path
        .to_str()
        .ok_or_else(|| anyhow!("Invalid UTF-8 in output path: {:?}", output_path))?;

    let output = Command::new(&gpg)
        .args(&["--armor", "--export", key_id, "--output", output_path_str])
        .output()
        .context("Failed to export GPG public key")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("Failed to export public key: {}", stderr);
    }

    println!("Exported public key to: {}", output_path.display());
    Ok(())
}
