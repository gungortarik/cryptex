using System;
using System.ComponentModel;
using System.IO.Pipes;
using System.Runtime.InteropServices;
using System.Security;
using System.Security.Principal;

// CI-only test helper: the server remains a separate process under its owner.
public static class WindowsAclProbe
{
    [DllImport("advapi32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    private static extern bool LogonUserW(string user, string domain, IntPtr password,
        int logonType, int provider, out IntPtr token);

    [DllImport("kernel32.dll", SetLastError = true)]
    private static extern bool CloseHandle(IntPtr handle);

    public sealed class Result
    {
        public string sid;
        public bool denied;
        public string exception;
        public int hresult;
    }

    public static Result Connect(string user, SecureString password, string expectedSid, string pipeName)
    {
        IntPtr token = IntPtr.Zero;
        IntPtr passwordBuffer = Marshal.SecureStringToGlobalAllocUnicode(password);
        try
        {
            // INTERACTIVE authenticates a distinct local identity; NEW_CREDENTIALS
            // would retain the caller's local identity and is not suitable here.
            if (!LogonUserW(user, ".", passwordBuffer, 2, 0, out token))
                throw new Win32Exception(Marshal.GetLastWin32Error(), "Test-account authentication failed.");
        }
        finally { Marshal.ZeroFreeGlobalAllocUnicode(passwordBuffer); }

        try
        {
            // Keep identity verification and the synchronous pipe open on this
            // same thread. Dispose restores the owner before returning to PowerShell.
            using (WindowsImpersonationContext context = WindowsIdentity.Impersonate(token))
            using (WindowsIdentity identity = WindowsIdentity.GetCurrent(true))
            {
                if (identity == null || identity.User.Value != expectedSid)
                    throw new InvalidOperationException("Client thread does not have the expected distinct-account SID.");
                Result result = new Result { sid = identity.User.Value };
                using (NamedPipeClientStream pipe = new NamedPipeClientStream(".", pipeName, PipeDirection.InOut))
                {
                    try { pipe.Connect(5000); }
                    catch (UnauthorizedAccessException error)
                    {
                        result.denied = true;
                        result.exception = error.GetType().FullName;
                        result.hresult = error.HResult;
                    }
                }
                return result;
            }
        }
        finally
        {
            if (!CloseHandle(token))
                throw new Win32Exception(Marshal.GetLastWin32Error(), "Test-account token cleanup failed.");
        }
    }
}
