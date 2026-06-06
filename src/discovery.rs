use netstat2::{get_sockets_info, AddressFamilyFlags, ProtocolFlags, ProtocolSocketInfo};
use sysinfo::System;

#[cfg(windows)]
use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
#[cfg(windows)]
use windows_sys::Win32::System::Diagnostics::Debug::ReadProcessMemory;
#[cfg(windows)]
use windows_sys::Win32::System::Memory::{MEM_COMMIT, MEMORY_BASIC_INFORMATION, VirtualQueryEx};
#[cfg(windows)]
use windows_sys::Win32::System::Threading::{
    OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ,
};

/// RAII wrapper that closes a Windows process handle on drop.
#[cfg(windows)]
struct OwnedHandle(HANDLE);

#[cfg(windows)]
impl Drop for OwnedHandle {
    fn drop(&mut self) {
        unsafe { CloseHandle(self.0); }
    }
}

// ── Private helpers ───────────────────────────────────────────────────────────

fn scan_processes() -> System {
    let mut sys = System::new_all();
    sys.refresh_all();
    sys
}

fn w3_game_pid(sys: &System) -> Option<u32> {
    sys.processes().iter().find_map(|(pid, process)| {
        let name = process.name().to_string_lossy().to_lowercase();
        (name.contains("warcraft iii") || name.contains("war3")).then_some(pid.as_u32())
    })
}

fn w3_all_pids(sys: &System) -> Vec<u32> {
    sys.processes()
        .iter()
        .filter_map(|(pid, process)| {
            let name = process.name().to_string_lossy().to_lowercase();
            (name.contains("warcraft iii")
                || name.contains("war3")
                || name.contains("blizzardbrowser"))
            .then_some(pid.as_u32())
        })
        .collect()
}

#[cfg(windows)]
fn guid_from_pid(pid: u32) -> crate::Result<String> {
    let raw_handle = unsafe { OpenProcess(PROCESS_VM_READ | PROCESS_QUERY_INFORMATION, 0, pid) };
    if raw_handle.is_null() {
        return Err(crate::Wc3Error::CannotOpenProcess);
    }
    let _handle = OwnedHandle(raw_handle);

    unsafe {
        let mut address: usize = 0;
        let mut mem_info: MEMORY_BASIC_INFORMATION = std::mem::zeroed();

        while VirtualQueryEx(
            raw_handle,
            address as *const _,
            &mut mem_info,
            std::mem::size_of::<MEMORY_BASIC_INFORMATION>(),
        ) != 0
        {
            if mem_info.State == MEM_COMMIT {
                let mut buffer = vec![0u8; mem_info.RegionSize];
                let mut bytes_read: usize = 0;

                if ReadProcessMemory(
                    raw_handle,
                    mem_info.BaseAddress,
                    buffer.as_mut_ptr() as *mut _,
                    buffer.len(),
                    &mut bytes_read,
                ) != 0
                {
                    buffer.truncate(bytes_read);

                    if let Some(pos) = find_subsequence(&buffer, b"?guid=") {
                        let start = pos + 6;
                        let end = digits_end(&buffer, start);
                        if end - start > 10 {
                            return Ok(String::from_utf8_lossy(&buffer[start..end]).into_owned());
                        }
                    }

                    if let Some(pos) = find_subsequence(&buffer, b"webui-socket/") {
                        let start = pos + 13;
                        let end = digits_end(&buffer, start);
                        if end - start > 10 {
                            return Ok(String::from_utf8_lossy(&buffer[start..end]).into_owned());
                        }
                    }
                }
            }
            address += mem_info.RegionSize;
        }
    }

    Err(crate::Wc3Error::GuidNotFound)
}

fn ports_from_pids(pids: &[u32]) -> crate::Result<Vec<u16>> {
    let af_flags = AddressFamilyFlags::IPV4 | AddressFamilyFlags::IPV6;
    let sockets_info = get_sockets_info(af_flags, ProtocolFlags::TCP)
        .map_err(|e| crate::Wc3Error::Netstat(e.to_string()))?;

    let mut ports = Vec::new();
    for si in sockets_info {
        if let ProtocolSocketInfo::Tcp(tcp_si) = si.protocol_socket_info {
            if tcp_si.state == netstat2::TcpState::Listen {
                let owned = si.associated_pids.iter().any(|pid| pids.contains(pid));
                if owned && !ports.contains(&tcp_si.local_port) {
                    ports.push(tcp_si.local_port);
                }
            }
        }
    }

    if ports.is_empty() {
        return Err(crate::Wc3Error::PortNotFound);
    }

    Ok(ports)
}

fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn digits_end(buf: &[u8], start: usize) -> usize {
    let mut end = start;
    while end < buf.len() && buf[end].is_ascii_digit() {
        end += 1;
    }
    end
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Scans the WC3 process memory to extract the WebSocket GUID.
///
/// Searches for the patterns `?guid=` or `webui-socket/` followed by a long digit sequence.
/// Takes ~1s since it scans the full virtual address space of the process.
#[cfg(windows)]
pub fn get_w3_guid() -> crate::Result<String> {
    let sys = scan_processes();
    let pid = w3_game_pid(&sys).ok_or(crate::Wc3Error::ProcessNotFound)?;
    guid_from_pid(pid)
}

/// Returns all TCP listening ports owned by the WC3 process (including BlizzardBrowser).
#[cfg(windows)]
pub fn get_w3_port() -> crate::Result<Vec<u16>> {
    let sys = scan_processes();
    let pids = w3_all_pids(&sys);
    if pids.is_empty() {
        return Err(crate::Wc3Error::ProcessNotFound);
    }
    ports_from_pids(&pids)
}

/// Single-pass discovery: returns `(ports, guid)` with only one process scan.
/// Used internally by `connect_auto`.
#[cfg(windows)]
pub(crate) fn discover() -> crate::Result<(Vec<u16>, String)> {
    let sys = scan_processes();
    let game_pid = w3_game_pid(&sys).ok_or(crate::Wc3Error::ProcessNotFound)?;
    let all_pids = w3_all_pids(&sys);
    let ports = ports_from_pids(&all_pids)?;
    let guid = guid_from_pid(game_pid)?;
    Ok((ports, guid))
}
