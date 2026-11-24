using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.Linq;
using System.Management.Automation;
using System.Reflection;
using System.Security.Cryptography;
using System.Security.Cryptography.X509Certificates;
using System.Security.Principal;
using System.Threading;
using System.Web.Script.Serialization;

internal static class Install
{
    static readonly JavaScriptSerializer Json = new JavaScriptSerializer();
    static readonly Dictionary<string, object> Result = new Dictionary<string, object>();
    static string state;

    static string Hash(Stream stream)
    {
        using (var sha = SHA256.Create()) return BitConverter.ToString(sha.ComputeHash(stream)).Replace("-", "");
    }

    static string FileHash(string path)
    {
        CheckPath(path);
        using (var stream = File.OpenRead(path)) return Hash(stream);
    }

    static void CheckPath(string path)
    {
        if ((File.GetAttributes(path) & FileAttributes.ReparsePoint) != 0)
            throw new IOException("Baglantili dosya veya klasor kullanilamaz.");
    }

    static T Cmd<T>(string command, params object[] pairs)
    {
        using (var shell = PowerShell.Create())
        {
            shell.AddCommand(command).AddParameter("ErrorAction", "Stop");
            for (int i = 0; i < pairs.Length; i += 2) shell.AddParameter((string)pairs[i], pairs[i + 1]);
            return (T)shell.Invoke().Single().BaseObject;
        }
    }

    static void Save(string path, string value)
    {
        string pending = path + "." + Guid.NewGuid().ToString("N");
        try
        {
            File.WriteAllText(pending, value);
            if (File.Exists(path)) { CheckPath(path); File.Replace(pending, path, null); }
            else File.Move(pending, path);
        }
        finally { File.Delete(pending); }
    }

    static X509Certificate2 Identity(string sid)
    {
        string subject = "CN=Cryptex M17 Trial " + sid;
        X509Certificate2 cert;
        using (var store = new X509Store(StoreName.My, StoreLocation.CurrentUser))
        {
            store.Open(OpenFlags.ReadOnly);
            cert = store.Certificates.Cast<X509Certificate2>().SingleOrDefault(c => c.Subject == subject);
        }
        if (cert == null)
            cert = Cmd<X509Certificate2>("New-SelfSignedCertificate", "Type", "CodeSigningCert",
                "Subject", subject, "CertStoreLocation", @"Cert:\CurrentUser\My",
                "Provider", "Microsoft Software Key Storage Provider", "KeyExportPolicy", "NonExportable",
                "HashAlgorithm", "SHA256", "NotAfter", DateTime.Now.AddDays(7));
        using (var key = cert.GetRSAPrivateKey() as RSACng)
        {
            if (key == null || key.Key.IsMachineKey || key.Key.ExportPolicy != CngExportPolicies.None ||
                cert.NotAfter <= DateTime.Now || cert.Subject != cert.Issuer ||
                cert.Extensions.OfType<X509BasicConstraintsExtension>().Any(e => e.CertificateAuthority) ||
                !cert.Extensions.OfType<X509EnhancedKeyUsageExtension>().SelectMany(e => e.EnhancedKeyUsages.Cast<Oid>())
                    .Select(o => o.Value).SequenceEqual(new[] { "1.3.6.1.5.5.7.3.3" }))
                throw new InvalidOperationException("Gecerli, disa aktarilamayan deneme imza anahtari gerekli.");
        }
        return cert;
    }

    static void Trust(X509Certificate2 cert)
    {
        using (var chain = new X509Chain())
        {
            chain.ChainPolicy.RevocationMode = X509RevocationMode.NoCheck;
            if (chain.Build(cert)) return;
        }
        Console.WriteLine("Windows onay isterse sertifika adini kontrol edin: " + cert.Subject);
        using (var store = new X509Store(StoreName.Root, StoreLocation.CurrentUser))
        using (var leaf = new X509Certificate2(cert.RawData))
        {
            store.Open(OpenFlags.ReadWrite);
            store.Add(leaf);
        }
    }

    static void Verify(string path, X509Certificate2 cert)
    {
        var sig = Cmd<Signature>("Get-AuthenticodeSignature", "LiteralPath", path);
        if (sig.Status != SignatureStatus.Valid || sig.SignatureType != SignatureType.Authenticode ||
            !sig.SignerCertificate.RawData.SequenceEqual(cert.RawData))
            throw new InvalidOperationException("Imza dogrulanamadi: " + Path.GetFileName(path) + " (" + sig.Status + ")");
    }

    static void Sign(string path, X509Certificate2 cert)
    {
        Cmd<Signature>("Set-AuthenticodeSignature", "FilePath", path, "Certificate", cert, "HashAlgorithm", "SHA256");
        Verify(path, cert);
    }

    static void SignInstalled(string path, string expected, X509Certificate2 cert)
    {
        string checkpoint = Path.Combine(state, Path.GetFileName(path) + ".sha256");
        string original = FileHash(path);
        if (File.Exists(checkpoint) && File.ReadAllText(checkpoint) == original)
        {
            Verify(path, cert);
            return;
        }
        if (original != expected) throw new InvalidDataException("Kurulan dosya derlemeden farkli: " + Path.GetFileName(path));
        string temp = Path.Combine(state, Guid.NewGuid().ToString("N") + ".exe");
        try
        {
            File.Copy(path, temp);
            if (FileHash(temp) != expected) throw new IOException("Kopyalama dogrulanamadi.");
            Sign(temp, cert);
            Save(checkpoint, FileHash(temp));
            File.Replace(temp, path, null);
            Verify(path, cert);
        }
        finally { File.Delete(temp); }
    }

    static void RunTrial(string sid)
    {
        var assembly = Assembly.GetExecutingAssembly();
        Dictionary<string, string> build;
        using (var reader = new StreamReader(assembly.GetManifestResourceStream("build.json")))
            build = Json.Deserialize<Dictionary<string, string>>(reader.ReadToEnd());
        Result["revision"] = build["revision"];
        string local = Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData);
        CheckPath(local);
        state = Directory.CreateDirectory(Path.Combine(local, "Cryptex-M17-Ilk-Kurulum")).FullName;
        CheckPath(state);
        string app = Path.Combine(state, "Application");
        string marker = Path.Combine(state, "release.txt");
        string identity = build["revision"] + " " + build["installer"];
        if (File.Exists(marker))
        {
            CheckPath(marker);
            if (File.ReadAllText(marker) != identity) throw new IOException("Bu deneme klasoru baska bir surume ait.");
        }
        else
        {
            if (Directory.EnumerateFileSystemEntries(state).Any()) throw new IOException("Deneme klasoru bos degil.");
            Save(marker, identity);
        }
        if (Directory.Exists(app)) CheckPath(app);
        string setup = Path.Combine(state, Guid.NewGuid().ToString("N") + ".exe");
        try
        {
            using (var source = assembly.GetManifestResourceStream("setup"))
            using (var dest = new FileStream(setup, FileMode.CreateNew)) source.CopyTo(dest);
            if (FileHash(setup) != build["installer"]) throw new InvalidDataException("Kurulum girdisi dogrulanamadi.");
            using (var cert = Identity(sid))
            {
                using (var bytes = new MemoryStream(cert.RawData)) Result["certificate_sha256"] = Hash(bytes);
                string pinFile = Path.Combine(state, "signer.sha256");
                string pin = (string)Result["certificate_sha256"];
                if (!File.Exists(pinFile)) Save(pinFile, pin);
                CheckPath(pinFile);
                if (File.ReadAllText(pinFile) != pin) throw new IOException("Deneme imza kimligi degisti.");
                Console.WriteLine("Cryptex deneme imzasi: " + Result["certificate_sha256"]);
                Trust(cert);
                Sign(setup, cert);
                Result["signed_installer_sha256"] = FileHash(setup);
                string desktop = Path.Combine(app, "cryptex-desktop.exe");
                string daemon = Path.Combine(app, "cryptexd.exe");
                string installed = Path.Combine(state, "installed.txt");
                if (!File.Exists(installed) || !File.Exists(desktop) || !File.Exists(daemon))
                {
                    Console.WriteLine("Dogrulanmis kurulum bu kullanicinin deneme klasorune yapiliyor...");
                    var start = new ProcessStartInfo(setup, "/S /D=" + app) { UseShellExecute = false };
                    using (var process = Process.Start(start))
                    {
                        if (!process.WaitForExit(180000)) { process.Kill(); throw new TimeoutException("Kurulum zaman asimi."); }
                        if (process.ExitCode != 0) throw new IOException("Kurulum cikis kodu: " + process.ExitCode);
                    }
                    Save(installed, identity);
                }
                CheckPath(app);
                SignInstalled(desktop, build["desktop"], cert);
                SignInstalled(daemon, build["daemon"], cert);
                Result["installed_desktop_sha256"] = FileHash(desktop);
                Result["installed_daemon_sha256"] = FileHash(daemon);
            }
        }
        finally { File.Delete(setup); }
    }

    [STAThread]
    static int Main()
    {
        int code = 1;
        try
        {
            var id = WindowsIdentity.GetCurrent();
            if (!String.Equals(id.Name, Environment.MachineName + @"\cryptexm14", StringComparison.OrdinalIgnoreCase) ||
                new WindowsPrincipal(id).IsInRole(WindowsBuiltInRole.Administrator))
                throw new InvalidOperationException("Windows VM'deki cryptexm14 hesabinda normal sekilde acin.");
            using (var mutex = new Mutex(false, @"Local\CryptexM17Trial-" + id.User.Value))
            {
                try { if (!mutex.WaitOne(0)) throw new IOException("Kurulum baska bir pencerede calisiyor."); }
                catch (AbandonedMutexException) { }
                try { RunTrial(id.User.Value); } finally { mutex.ReleaseMutex(); }
            }
            Result["passed"] = true;
            Console.WriteLine("TAMAM. Kurulum, masaustu ve servis imzalari dogrulandi.");
            code = 0;
        }
        catch (Exception e) { Result["passed"] = false; Result["error"] = e.Message; Console.WriteLine(e.Message); }
        try
        {
            string report = Path.Combine(AppDomain.CurrentDomain.BaseDirectory, "M17-IMZA-SONUC.json");
            Save(report, Json.Serialize(Result));
            Console.WriteLine("Sonuc: " + report);
        }
        catch (Exception e) { Console.WriteLine("Sonuc yazilamadi: " + e.Message); code = 1; }
        if (!Console.IsInputRedirected && !Console.IsOutputRedirected) { Console.WriteLine("Enter ile kapatin."); Console.ReadLine(); }
        return code;
    }
}
