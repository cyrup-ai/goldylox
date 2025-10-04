# STUB_5: Remove macOS Daemon Installation Import Stub

---

## ⚠️ CRITICAL ARCHITECTURE NOTE

**NO Arc<T> ALLOWED IN THIS CODEBASE**

This system uses:
- **Crossbeam channels** for message passing
- **DashMap** for concurrent data structures
- **Worker-owned data** - workers OWN all state, no sharing

**Any solution using Arc<T> violates core architecture and will be rejected.**

Pattern: Send messages via channels. Workers own receivers and all data.

---


## OBJECTIVE
Investigate and remove the "for now" stub comment in the macOS daemon installation imports section. Determine if this is just a leftover comment or if functionality is missing.

---

## CURRENT PROBLEM

**File:** `src/daemon/install/macos.rs`  
**Line:** 10

**Current Code:**
```rust
use anyhow::{Context, Result};
use once_cell::sync::OnceCell;
for now  // ❌ REMOVE THIS STUB

use crate::daemon::install::builder::CommandBuilder;
use crate::daemon::install::{InstallerBuilder, InstallerError};
```

**Impact:** Unclear - could be a leftover comment from incomplete development, or could indicate missing imports/functionality.

---

## SUBTASK 1: Investigate the Stub Context

**Research Required:**

**File:** `src/daemon/install/macos.rs`
- Read the entire file to understand what it does
- Check if all necessary imports are present
- Verify if any functionality is missing
- Look for other comments suggesting incomplete work

**Questions to Answer:**
1. Is this just a leftover comment with no missing functionality?
2. Are there missing imports that should be added?
3. Is there incomplete macOS daemon installation logic?
4. Does the file compile and work correctly despite the comment?

**Check For:**
- Unused imports (imports present but not used)
- Missing imports (functionality used but not imported)
- Incomplete implementations elsewhere in the file
- References to "TODO", "FIXME", or other stub markers

---

## SUBTASK 2: Verify Functionality Completeness

**What to Check:**

**Installation Functions:**
- Is daemon installation fully implemented for macOS?
- Are all necessary system calls present?
- Is LaunchAgent/LaunchDaemon setup complete?

**Import Completeness:**
- Are all used types/functions imported?
- Are there any compiler warnings about unused imports?
- Are there any missing imports that would improve the code?

**Comparison with Other Platforms:**
- Compare with `src/daemon/install/linux.rs`
- Compare with `src/daemon/install/windows.rs`
- Is macOS implementation at feature parity?

**Expected macOS Installation Components:**
- LaunchAgent/LaunchDaemon plist file generation
- Installation to `/Library/LaunchDaemons/` or `~/Library/LaunchAgents/`
- Binary placement in appropriate location
- Permissions setup
- Service registration with `launchctl`

---

## SUBTASK 3: Remove Stub or Complete Implementation

**Scenario A: Just a Leftover Comment (Most Likely)**

If investigation shows all functionality is complete:
1. Simply remove the "for now" comment
2. Verify code compiles: `cargo check`
3. Done!

**Scenario B: Missing Imports**

If investigation shows missing imports:
1. Identify what needs to be imported
2. Add the missing import statements
3. Remove the "for now" comment
4. Verify code compiles: `cargo check`

**Scenario C: Incomplete Functionality**

If investigation shows incomplete daemon installation:
1. Document what's missing
2. Implement missing functionality
3. Add necessary imports
4. Remove the "for now" comment
5. Verify code compiles: `cargo check`

---

## SUBTASK 4: Code Cleanup

**After Removing Stub:**

**Check for Unused Imports:**
```bash
cargo clippy --allow-dirty 2>&1 | grep "unused"
```

**Verify No Other Stubs in File:**
```bash
grep -i "for now\|todo\|fixme" src/daemon/install/macos.rs
```

**Ensure Consistent Style:**
- Import ordering matches other files in the module
- Comments are clear and professional
- No other placeholder comments remain

---

## DEFINITION OF DONE

**Verification Steps:**
1. "for now" comment removed from `macos.rs:10`
2. All necessary imports are present
3. No missing functionality identified
4. Code compiles: `cargo check`
5. No compiler warnings about unused imports
6. No other stub comments in the file

**Success Criteria:**
- Clean import section with no placeholder comments
- All imports used or removed
- macOS daemon installation functionality complete
- Professional, production-ready code
- No leftover development comments

---

## RESEARCH NOTES

**macOS Daemon Installation:**

**Relevant Files:**
- `src/daemon/install/macos.rs` - macOS installation implementation
- `src/daemon/install/builder.rs` - Command builder utilities
- `src/daemon/install/mod.rs` - Installation module interface
- `src/daemon/install/linux.rs` - Linux comparison
- `src/daemon/install/windows.rs` - Windows comparison

**macOS Service Management:**
- LaunchAgent: User-level services
- LaunchDaemon: System-level services
- Plist format: XML property list for service configuration
- launchctl: Command-line tool for service management
- Installation paths:
  - System: `/Library/LaunchDaemons/`
  - User: `~/Library/LaunchAgents/`

**Expected Imports for macOS Installation:**
- File I/O (reading/writing plist files)
- Process spawning (calling `launchctl`)
- Path manipulation (installation directories)
- Error handling (Result, Context, anyhow)
- Platform-specific APIs if needed

---

## IMPLEMENTATION CONSTRAINTS

**DO NOT:**
- ❌ Write unit tests (separate team handles testing)
- ❌ Write integration tests (separate team handles testing)
- ❌ Write benchmarks (separate team handles benchmarks)
- ❌ Add functionality beyond what's necessary
- ❌ Change working code unnecessarily

**DO:**
- ✅ Focus solely on removing stub comment
- ✅ Verify functionality is complete
- ✅ Add missing imports if needed
- ✅ Clean up unused imports if present
- ✅ Ensure code compiles cleanly
- ✅ Follow existing code style
- ✅ Document any significant findings

---

## NOTES

**Complexity:** Low
- Likely just a leftover comment to remove
- Minimal code changes expected
- Quick investigation and cleanup task
- May require no changes beyond removing the comment

**Session Focus:** Investigation and cleanup of macos.rs import stub.

**Expected Outcome:** 
Most likely this is just a leftover comment from development. Remove it, verify compilation, and close the task. If missing functionality is discovered, implement the minimum necessary to complete the daemon installation feature.

**Time Estimate:** Short session - investigation + simple fix.

**Approach:**
1. Read entire macos.rs file (likely <500 lines)
2. Verify all imports are used
3. Check if any functionality is incomplete
4. Remove stub comment
5. Clean compile verification
