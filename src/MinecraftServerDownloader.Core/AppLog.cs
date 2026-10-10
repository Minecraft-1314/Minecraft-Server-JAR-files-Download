using System.Diagnostics;
using System.Text;

namespace MinecraftServerDownloader.Core;

public static class AppLog
{
    private static readonly object Sync = new();
    private static StreamWriter? _writer;
    private static bool _bound;

    public static string LogDirectory { get; } = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "MinecraftServerDownloader", "logs");

    public static void Bind()
    {
        lock (Sync)
        {
            if (_bound) return;
            _bound = true;
            try
            {
                Directory.CreateDirectory(LogDirectory);
                FileStream stream = new(Path.Combine(LogDirectory, "app.log"), FileMode.Append, FileAccess.Write, FileShare.Read);
                _writer = new StreamWriter(stream, Encoding.UTF8) { AutoFlush = true };
                RotateIfNeeded();
            }
            catch { _writer = null; }
        }
    }

    public static void Info(string message) => Write("INFO", message);

    public static void Warn(string message) => Write("WARN", message);

    public static void HandleUnhandled(Exception error) => Write("ERROR", error.ToString());

    private static void Write(string level, string value)
    {
        string safe = Mask(value);
        lock (Sync)
        {
            if (_writer is null) return;
            try { _writer.WriteLine($"{DateTimeOffset.UtcNow:O} [{level}] {safe}"); }
            catch { _writer.Dispose(); _writer = null; }
        }
    }

    private static string Mask(string value)
    {
        string masked = System.Text.RegularExpressions.Regex.Replace(value, @"(?i)(authorization|token|password|secret|credential)\s*[:=]\s*\S+", "$1=***");
        masked = System.Text.RegularExpressions.Regex.Replace(masked, @"(?i)([?&](?:key|token|password|secret)=)[^&\s]+", "$1***");
        return masked.Length > 12_000 ? masked[..12_000] + "..." : masked;
    }

    private static void RotateIfNeeded()
    {
        string current = Path.Combine(LogDirectory, "app.log");
        FileInfo info = new(current);
        if (!info.Exists || info.Length <= 1_048_576) return;
        try { info.MoveTo(current + "." + DateTime.UtcNow.ToString("yyyyMMddHHmmss", System.Globalization.CultureInfo.InvariantCulture)); } catch { }
    }
}

