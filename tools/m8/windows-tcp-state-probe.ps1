<#
Read-only native TCP state diagnostic. Creates only temporary loopback sockets;
does not run Cryptex, read profiles, send credentials, or change network settings.
Run with -SelfTest to verify malformed-table rejection without native sockets.
Native results are diagnostic evidence, not authorization to relax ownership.
#>
[CmdletBinding()]
param(
    [switch]$SelfTest,
    [switch]$Summary,
    [ValidateRange(1, 20)][int]$Iterations = 3
)
$ErrorActionPreference = 'Stop'

Add-Type -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.Net;
using System.Net.Sockets;
using System.Runtime.InteropServices;

public static class CryptexTcpStateProbe {
    [DllImport("iphlpapi.dll")]
    static extern uint GetExtendedTcpTable(IntPtr table, ref uint size,
        bool order, uint family, int kind, uint reserved);

    public sealed class Row {
        public uint State;
        public uint ProcessId;
    }
    public sealed class Observation {
        public string Family;
        public int Iteration;
        public int Sample;
        public int ExpectedProcessId;
        public bool RoundTripPassed;
        public bool RoundTripAfterSnapshotPassed;
        public Row[] Rows;
        public bool ExactEstablishedOwner;
    }

    static bool SameBytes(byte[] table, int offset, byte[] address) {
        for (int i = 0; i < address.Length; i++)
            if (table[offset + i] != address[i]) return false;
        return true;
    }
    static int Port(byte[] table, int offset) {
        return (table[offset] << 8) | table[offset + 1];
    }
    public static Row[] Parse(byte[] table, bool ipv6, int serverPort, int clientPort) {
        if (table.Length < 4) throw new InvalidOperationException("truncated_count");
        uint count = BitConverter.ToUInt32(table, 0);
        int stride = ipv6 ? 56 : 24;
        if (count > (table.Length - 4) / stride)
            throw new InvalidOperationException("truncated_rows");
        byte[] address = (ipv6 ? IPAddress.IPv6Loopback : IPAddress.Loopback).GetAddressBytes();
        var rows = new List<Row>();
        for (int i = 0; i < count; i++) {
            int at = 4 + i * stride;
            int localAddress = at + (ipv6 ? 0 : 4);
            int remoteAddress = at + (ipv6 ? 24 : 12);
            int localPort = at + (ipv6 ? 20 : 8);
            int remotePort = at + (ipv6 ? 44 : 16);
            if (Port(table, localPort) != serverPort || Port(table, remotePort) != clientPort
                || !SameBytes(table, localAddress, address)
                || !SameBytes(table, remoteAddress, address)) continue;
            if (ipv6 && (BitConverter.ToUInt32(table, at + 16) != 0
                || BitConverter.ToUInt32(table, at + 40) != 0)) continue;
            rows.Add(new Row {
                State = BitConverter.ToUInt32(table, at + (ipv6 ? 48 : 0)),
                ProcessId = BitConverter.ToUInt32(table, at + (ipv6 ? 52 : 20))
            });
        }
        return rows.ToArray();
    }
    static byte[] Capture(bool ipv6) {
        uint family = ipv6 ? 23u : 2u;
        uint size = 0;
        uint status = GetExtendedTcpTable(IntPtr.Zero, ref size, false, family, 5, 0);
        if (status != 122) throw new InvalidOperationException("table_size_status_" + status);
        for (int attempt = 0; attempt < 3; attempt++) {
            if (size < 4 || size > 4 * 1024 * 1024)
                throw new InvalidOperationException("table_size_out_of_bounds");
            int capacity = (int)size;
            IntPtr buffer = Marshal.AllocHGlobal(capacity);
            try {
                Marshal.Copy(new byte[capacity], 0, buffer, capacity);
                status = GetExtendedTcpTable(buffer, ref size, false, family, 5, 0);
                if (status == 122) continue;
                if (status != 0) throw new InvalidOperationException("table_status_" + status);
                if (size < 4 || size > capacity)
                    throw new InvalidOperationException("table_returned_size_out_of_bounds");
                byte[] bytes = new byte[size];
                Marshal.Copy(buffer, bytes, 0, bytes.Length);
                return bytes;
            } finally { Marshal.FreeHGlobal(buffer); }
        }
        throw new InvalidOperationException("table_grew_repeatedly");
    }
    static bool RoundTrip(Socket client, Socket server) {
        byte[] data = new byte[1];
        if (client.Send(new byte[] { 75 }) != 1
            || server.Receive(data) != 1 || data[0] != 75) return false;
        return server.Send(new byte[] { 76 }) == 1
            && client.Receive(data) == 1 && data[0] == 76;
    }
    public static Observation[] Run(int iterations) {
        var results = new List<Observation>();
        int processId = System.Diagnostics.Process.GetCurrentProcess().Id;
        foreach (bool ipv6 in new[] { false, true }) {
            var family = ipv6 ? AddressFamily.InterNetworkV6 : AddressFamily.InterNetwork;
            var address = ipv6 ? IPAddress.IPv6Loopback : IPAddress.Loopback;
            for (int iteration = 0; iteration < iterations; iteration++) {
                using (var listener = new Socket(family, SocketType.Stream, ProtocolType.Tcp))
                using (var client = new Socket(family, SocketType.Stream, ProtocolType.Tcp)) {
                    listener.Bind(new IPEndPoint(address, 0));
                    listener.Listen(1);
                    client.Connect(listener.LocalEndPoint);
                    using (var server = listener.Accept()) {
                        client.ReceiveTimeout = server.ReceiveTimeout = 1000;
                        client.SendTimeout = server.SendTimeout = 1000;
                        for (int sample = 0; sample < 3; sample++) {
                            bool roundTrip = RoundTrip(client, server);
                            var rows = Parse(Capture(ipv6), ipv6,
                                ((IPEndPoint)server.LocalEndPoint).Port,
                                ((IPEndPoint)client.LocalEndPoint).Port);
                            bool roundTripAfter = RoundTrip(client, server);
                            results.Add(new Observation {
                                Family = ipv6 ? "IPv6" : "IPv4", Iteration = iteration,
                                Sample = sample, ExpectedProcessId = processId,
                                RoundTripPassed = roundTrip, Rows = rows,
                                RoundTripAfterSnapshotPassed = roundTripAfter,
                                ExactEstablishedOwner = roundTrip && roundTripAfter && rows.Length == 1
                                    && rows[0].State == 5 && rows[0].ProcessId == processId
                            });
                            System.Threading.Thread.Sleep(10);
                        }
                    }
                }
            }
        }
        return results.ToArray();
    }
    public static string SelfTest() {
        foreach (bool ipv6 in new[] { false, true }) {
            foreach (var bytes in new[] { new byte[0], new byte[3], new byte[] { 255,255,255,255 } }) {
                bool rejected = false;
                try { Parse(bytes, ipv6, 8384, 50000); }
                catch (InvalidOperationException) { rejected = true; }
                if (!rejected) throw new Exception("malformed_table_accepted");
            }
            int stride = ipv6 ? 56 : 24;
            byte[] table = new byte[4 + stride * 2];
            table[0] = 2;
            for (int i = 0; i < 2; i++) {
                int at = 4 + i * stride;
                byte[] address = (ipv6 ? IPAddress.IPv6Loopback : IPAddress.Loopback).GetAddressBytes();
                address.CopyTo(table, at + (ipv6 ? 0 : 4));
                address.CopyTo(table, at + (ipv6 ? 24 : 12));
                table[at + (ipv6 ? 20 : 8)] = 32; table[at + (ipv6 ? 21 : 9)] = 192;
                table[at + (ipv6 ? 44 : 16)] = 195; table[at + (ipv6 ? 45 : 17)] = 80;
                BitConverter.GetBytes((uint)(i == 0 ? 5 : 1)).CopyTo(table, at + (ipv6 ? 48 : 0));
                BitConverter.GetBytes(1234u).CopyTo(table, at + (ipv6 ? 52 : 20));
            }
            var rows = Parse(table, ipv6, 8384, 50000);
            if (rows.Length != 2 || rows[0].State != 5 || rows[1].State != 1
                || rows[0].ProcessId != 1234) throw new Exception("duplicate_or_state_lost");
            if (Parse(table, ipv6, 8384, 50001).Length != 0)
                throw new Exception("wrong_tuple_accepted");
        }
        return "PASS: IPv4/IPv6 malformed bounds, exact tuple, duplicate rows and raw states";
    }
}
'@

if ($SelfTest) {
    [CryptexTcpStateProbe]::SelfTest()
    exit 0
}
$observations = @([CryptexTcpStateProbe]::Run($Iterations))
$mismatches = @($observations | Where-Object { -not $_.ExactEstablishedOwner })
$windowsVersion = Get-ItemProperty -LiteralPath 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion'
$report = [ordered]@{
    scope = 'synthetic loopback only; not a Cryptex or peer-trust pass'
    os = [Environment]::OSVersion.Version.ToString()
    build_revision = "$($windowsVersion.CurrentBuild).$($windowsVersion.UBR)"
    display_version = $windowsVersion.DisplayVersion
    expected_state = 5
    samples = $observations.Count
    mismatches = $mismatches.Count
    round_trip_samples_before_and_after = @($observations | Where-Object {
        $_.RoundTripPassed -and $_.RoundTripAfterSnapshotPassed
    }).Count
    observed_states = @($observations | ForEach-Object { $_.Rows } |
        ForEach-Object { $_.State } | Sort-Object -Unique)
    mismatch_families = @($mismatches | ForEach-Object { $_.Family } | Sort-Object -Unique)
}
if (-not $Summary) { $report.observations = $observations }
$report | ConvertTo-Json -Depth 7
if ($mismatches.Count -ne 0) { exit 1 }
