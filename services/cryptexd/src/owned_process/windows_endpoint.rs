use std::mem::{offset_of, size_of};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::ptr;

use windows_sys::Win32::Foundation::{ERROR_INSUFFICIENT_BUFFER, WAIT_TIMEOUT};
use windows_sys::Win32::NetworkManagement::IpHelper::{
    GetExtendedTcpTable, MIB_TCP_STATE_ESTAB, MIB_TCP6ROW_OWNER_PID, MIB_TCP6TABLE_OWNER_PID,
    MIB_TCPROW_OWNER_PID, MIB_TCPTABLE_OWNER_PID, TCP_TABLE_OWNER_PID_ALL,
};
use windows_sys::Win32::Networking::WinSock::{AF_INET, AF_INET6};
use windows_sys::Win32::System::JobObjects::IsProcessInJob;
use windows_sys::Win32::System::Threading::{
    OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE, WaitForSingleObject,
};

use super::super::endpoint::EndpointError;
use super::{Handle, NativeProcess};

const MAX_TABLE: usize = 4 * 1024 * 1024;

pub(crate) struct PeerHandle {
    _process: Handle,
}

impl NativeProcess {
    pub(crate) fn require_root_alive(&self) -> Result<(), EndpointError> {
        if unsafe { WaitForSingleObject(self.process.0, 0) } != WAIT_TIMEOUT {
            return Err(EndpointError::OwnerExited);
        }
        Ok(())
    }

    pub(crate) fn retain_owned_peer(&self, pid: u32) -> Result<PeerHandle, EndpointError> {
        let handle = unsafe {
            OpenProcess(
                PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE,
                0,
                pid,
            )
        };
        if handle.is_null() {
            return Err(EndpointError::PeerNotOwned);
        }
        let handle = Handle(handle);
        let mut member = 0;
        if unsafe { IsProcessInJob(handle.0, self.job.0, &mut member) } == 0
            || member == 0
            || unsafe { WaitForSingleObject(handle.0, 0) } != WAIT_TIMEOUT
        {
            return Err(EndpointError::PeerNotOwned);
        }
        Ok(PeerHandle { _process: handle })
    }
}

pub(crate) fn connected_server_pid(
    client: SocketAddr,
    server: SocketAddr,
) -> Result<u32, EndpointError> {
    if client.is_ipv4() != server.is_ipv4() {
        return Err(EndpointError::InvalidEndpoint);
    }
    let family = if server.is_ipv4() { AF_INET } else { AF_INET6 };
    let mut required = 0;
    let status = unsafe {
        GetExtendedTcpTable(
            ptr::null_mut(),
            &mut required,
            0,
            family as u32,
            TCP_TABLE_OWNER_PID_ALL,
            0,
        )
    };
    if status != ERROR_INSUFFICIENT_BUFFER {
        return Err(EndpointError::TableUnavailable);
    }
    // A changing table may grow between the size and data calls. Bounded retry;
    // no sleeping, port-only fallback or unbounded allocation on failure.
    for _ in 0..3 {
        if required as usize > MAX_TABLE {
            return Err(EndpointError::TableTooLarge);
        }
        if required < 4 {
            return Err(EndpointError::InvalidTable);
        }
        let mut storage = vec![0u32; (required as usize).div_ceil(4)];
        let mut available = (storage.len() * 4) as u32;
        let status = unsafe {
            GetExtendedTcpTable(
                storage.as_mut_ptr().cast(),
                &mut available,
                0,
                family as u32,
                TCP_TABLE_OWNER_PID_ALL,
                0,
            )
        };
        if status == ERROR_INSUFFICIENT_BUFFER {
            required = available;
            continue;
        }
        if status != 0 {
            return Err(EndpointError::TableUnavailable);
        }
        if available as usize > storage.len() * 4 || available < 4 {
            return Err(EndpointError::InvalidTable);
        }
        return find_server(&storage, available as usize, client, server);
    }
    Err(EndpointError::TableUnavailable)
}

fn find_server(
    storage: &[u32],
    available: usize,
    client: SocketAddr,
    server: SocketAddr,
) -> Result<u32, EndpointError> {
    let count = storage
        .first()
        .copied()
        .ok_or(EndpointError::InvalidTable)? as usize;
    let (offset, stride) = if server.is_ipv4() {
        (
            offset_of!(MIB_TCPTABLE_OWNER_PID, table),
            size_of::<MIB_TCPROW_OWNER_PID>(),
        )
    } else {
        (
            offset_of!(MIB_TCP6TABLE_OWNER_PID, table),
            size_of::<MIB_TCP6ROW_OWNER_PID>(),
        )
    };
    let end = count
        .checked_mul(stride)
        .and_then(|n| n.checked_add(offset))
        .ok_or(EndpointError::InvalidTable)?;
    if end > available || available > storage.len() * 4 {
        return Err(EndpointError::InvalidTable);
    }
    let bytes = storage.as_ptr().cast::<u8>();
    let mut found = None;
    for index in 0..count {
        let row = unsafe { bytes.add(offset + index * stride) };
        let candidate = if server.is_ipv4() {
            // Bounds above cover each complete SDK structure. Native addresses
            // use network bytes in DWORD storage; ports occupy its low WORD.
            let row = unsafe { ptr::read_unaligned(row.cast::<MIB_TCPROW_OWNER_PID>()) };
            let local = SocketAddr::new(
                IpAddr::V4(Ipv4Addr::from(row.dwLocalAddr.to_ne_bytes())),
                u16::from_be(row.dwLocalPort as u16),
            );
            let remote = SocketAddr::new(
                IpAddr::V4(Ipv4Addr::from(row.dwRemoteAddr.to_ne_bytes())),
                u16::from_be(row.dwRemotePort as u16),
            );
            (row.dwState == MIB_TCP_STATE_ESTAB as u32 && local == server && remote == client)
                .then_some(row.dwOwningPid)
        } else {
            let row = unsafe { ptr::read_unaligned(row.cast::<MIB_TCP6ROW_OWNER_PID>()) };
            let local = SocketAddr::new(
                IpAddr::V6(Ipv6Addr::from(row.ucLocalAddr)),
                u16::from_be(row.dwLocalPort as u16),
            );
            let remote = SocketAddr::new(
                IpAddr::V6(Ipv6Addr::from(row.ucRemoteAddr)),
                u16::from_be(row.dwRemotePort as u16),
            );
            (row.dwState == MIB_TCP_STATE_ESTAB as u32
                && row.dwLocalScopeId == 0
                && row.dwRemoteScopeId == 0
                && local == server
                && remote == client)
                .then_some(row.dwOwningPid)
        };
        if let Some(pid) = candidate {
            if pid == 0 || found.is_some() {
                return Err(EndpointError::InvalidTable);
            }
            found = Some(pid);
        }
    }
    found.ok_or(EndpointError::PeerNotOwned)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ipv4_table(rows: &[MIB_TCPROW_OWNER_PID]) -> Vec<u32> {
        let mut table = vec![0u32; 1 + rows.len() * 6];
        table[0] = rows.len() as u32;
        // All fields are DWORDs; copy initialized SDK rows after the count.
        unsafe {
            ptr::copy_nonoverlapping(
                rows.as_ptr().cast::<u32>(),
                table.as_mut_ptr().add(1),
                rows.len() * 6,
            );
        }
        table
    }

    fn row(remote_port: u16) -> MIB_TCPROW_OWNER_PID {
        MIB_TCPROW_OWNER_PID {
            dwState: MIB_TCP_STATE_ESTAB as u32,
            dwLocalAddr: u32::from_ne_bytes([127, 0, 0, 1]),
            dwLocalPort: u16::to_be(8384) as u32,
            dwRemoteAddr: u32::from_ne_bytes([127, 0, 0, 1]),
            dwRemotePort: remote_port.to_be() as u32,
            dwOwningPid: 1234,
        }
    }

    #[test]
    fn tcp_table_matches_exact_established_tuple_not_only_port() {
        let client = "127.0.0.1:50001".parse().unwrap();
        let server = "127.0.0.1:8384".parse().unwrap();
        let table = ipv4_table(&[row(50000), row(50001)]);
        assert_eq!(
            find_server(&table, table.len() * 4, client, server).unwrap(),
            1234
        );
        let table = ipv4_table(&[row(50000)]);
        assert_eq!(
            find_server(&table, table.len() * 4, client, server).unwrap_err(),
            EndpointError::PeerNotOwned
        );
    }

    #[test]
    fn tcp_table_duplicate_truncated_and_count_overflow_fail_closed() {
        let client = "127.0.0.1:50001".parse().unwrap();
        let server = "127.0.0.1:8384".parse().unwrap();
        let table = ipv4_table(&[row(50001), row(50001)]);
        assert_eq!(
            find_server(&table, table.len() * 4, client, server).unwrap_err(),
            EndpointError::InvalidTable
        );
        assert_eq!(
            find_server(&table, 4, client, server).unwrap_err(),
            EndpointError::InvalidTable
        );
        assert_eq!(
            find_server(&[u32::MAX], 4, client, server).unwrap_err(),
            EndpointError::InvalidTable
        );
    }

    #[test]
    fn non_established_or_zero_pid_is_not_authority() {
        let client = "127.0.0.1:50001".parse().unwrap();
        let server = "127.0.0.1:8384".parse().unwrap();
        let mut value = row(50001);
        value.dwState = 2;
        let table = ipv4_table(&[value]);
        assert_eq!(
            find_server(&table, table.len() * 4, client, server).unwrap_err(),
            EndpointError::PeerNotOwned
        );
        value = row(50001);
        value.dwOwningPid = 0;
        let table = ipv4_table(&[value]);
        assert_eq!(
            find_server(&table, table.len() * 4, client, server).unwrap_err(),
            EndpointError::InvalidTable
        );
    }
}
